use std::collections::VecDeque;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use pulseseek_domain::analysis::AnalysisBlock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanePolicy {
    Continuous,
    LatestOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaneValidity {
    Measured,
    Complete,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmissionResult {
    Accepted,
    DroppedVisual,
    ContinuousGap,
    ReceiverGone,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LaneDiagnostics {
    pub input_depth: usize,
    pub input_high_water: usize,
    pub output_depth: usize,
    pub output_high_water: usize,
    pub visual_drops: u64,
    pub stale_visual_inputs: u64,
    pub continuous_gaps: u64,
    pub processor_panics: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaneError {
    InvalidCapacity,
    WorkerStartFailed,
    WorkerPanicked,
}

impl fmt::Display for LaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity => formatter.write_str("lane capacities must be positive"),
            Self::WorkerStartFailed => formatter.write_str("analysis lane worker could not start"),
            Self::WorkerPanicked => formatter.write_str("analysis lane worker panicked"),
        }
    }
}

impl std::error::Error for LaneError {}

struct Mailbox<T> {
    queue: Mutex<VecDeque<T>>,
    available: Arc<Condvar>,
    capacity: usize,
}

impl<T> Mailbox<T> {
    fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            available: Arc::new(Condvar::new()),
            capacity,
        }
    }
}

struct LaneState {
    shutdown: AtomicBool,
    sender_connected: AtomicBool,
    receiver_connected: AtomicBool,
    incomplete: AtomicBool,
    input_depth: AtomicUsize,
    input_high_water: AtomicUsize,
    output_depth: AtomicUsize,
    output_high_water: AtomicUsize,
    visual_drops: AtomicU64,
    stale_visual_inputs: AtomicU64,
    continuous_gaps: AtomicU64,
    processor_panics: AtomicU64,
}

impl LaneState {
    fn diagnostics(&self) -> LaneDiagnostics {
        LaneDiagnostics {
            input_depth: self.input_depth.load(Ordering::Acquire),
            input_high_water: self.input_high_water.load(Ordering::Relaxed),
            output_depth: self.output_depth.load(Ordering::Acquire),
            output_high_water: self.output_high_water.load(Ordering::Relaxed),
            visual_drops: self.visual_drops.load(Ordering::Relaxed),
            stale_visual_inputs: self.stale_visual_inputs.load(Ordering::Relaxed),
            continuous_gaps: self.continuous_gaps.load(Ordering::Relaxed),
            processor_panics: self.processor_panics.load(Ordering::Relaxed),
        }
    }
}

pub struct AnalysisLaneSender {
    input: Arc<Mailbox<AnalysisBlock>>,
    output: Arc<MailboxErased>,
    policy: LanePolicy,
    state: Arc<LaneState>,
}

struct MailboxErased {
    available: Arc<Condvar>,
}

impl AnalysisLaneSender {
    pub fn try_submit(&self, block: AnalysisBlock) -> SubmissionResult {
        if self.state.shutdown.load(Ordering::Acquire) {
            return SubmissionResult::Shutdown;
        }
        if !self.state.receiver_connected.load(Ordering::Acquire) {
            return SubmissionResult::ReceiverGone;
        }
        let mut queue = self.input.queue.lock().unwrap_or_else(|error| error.into_inner());
        if queue.len() == self.input.capacity {
            match self.policy {
                LanePolicy::LatestOnly => {
                    queue.pop_front();
                    queue.push_back(block);
                    self.state.visual_drops.fetch_add(1, Ordering::Relaxed);
                    self.input.available.notify_one();
                    SubmissionResult::DroppedVisual
                },
                LanePolicy::Continuous => self.record_continuous_gap(),
            }
        } else {
            queue.push_back(block);
            let depth = queue.len();
            self.state.input_depth.store(depth, Ordering::Release);
            self.state.input_high_water.fetch_max(depth, Ordering::Relaxed);
            self.input.available.notify_one();
            SubmissionResult::Accepted
        }
    }

    fn record_continuous_gap(&self) -> SubmissionResult {
        self.state.continuous_gaps.fetch_add(1, Ordering::Relaxed);
        self.state.incomplete.store(true, Ordering::Release);
        SubmissionResult::ContinuousGap
    }

    pub fn diagnostics(&self) -> LaneDiagnostics {
        self.state.diagnostics()
    }

    pub fn shutdown(&self) {
        self.state.shutdown.store(true, Ordering::Release);
        self.input.available.notify_all();
        self.output.available.notify_all();
    }
}

impl Drop for AnalysisLaneSender {
    fn drop(&mut self) {
        self.state.sender_connected.store(false, Ordering::Release);
        self.input.available.notify_all();
    }
}

#[derive(Debug)]
pub struct LaneOutput<T> {
    pub value: T,
    pub validity: LaneValidity,
}

pub struct AnalysisLaneReceiver<T> {
    output: Arc<Mailbox<LaneOutput<T>>>,
    input: Arc<MailboxErased>,
    state: Arc<LaneState>,
}

impl<T> AnalysisLaneReceiver<T> {
    pub fn try_receive(&self) -> Option<LaneOutput<T>> {
        let mut queue = self.output.queue.lock().unwrap_or_else(|error| error.into_inner());
        let value = queue.pop_front();
        self.state.output_depth.store(queue.len(), Ordering::Release);
        value
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<LaneOutput<T>, RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        let mut queue = self.output.queue.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(value) = queue.pop_front() {
                self.state.output_depth.store(queue.len(), Ordering::Release);
                return Ok(value);
            }
            if self.state.shutdown.load(Ordering::Acquire)
                || !self.state.sender_connected.load(Ordering::Acquire)
            {
                return Err(RecvTimeoutError::Disconnected);
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(RecvTimeoutError::Timeout);
            };
            let (next, wait) = self
                .output
                .available
                .wait_timeout(queue, remaining)
                .unwrap_or_else(|error| error.into_inner());
            queue = next;
            if wait.timed_out() && queue.is_empty() {
                return Err(RecvTimeoutError::Timeout);
            }
        }
    }

    pub fn diagnostics(&self) -> LaneDiagnostics {
        self.state.diagnostics()
    }
}

impl<T> Drop for AnalysisLaneReceiver<T> {
    fn drop(&mut self) {
        self.state.receiver_connected.store(false, Ordering::Release);
        let mut queue = self.output.queue.lock().unwrap_or_else(|error| error.into_inner());
        queue.clear();
        self.state.output_depth.store(0, Ordering::Release);
        self.input.available.notify_all();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecvTimeoutError {
    Timeout,
    Disconnected,
}

pub struct AnalysisLaneWorker {
    state: Arc<LaneState>,
    input: Arc<MailboxErased>,
    output: Arc<MailboxErased>,
    join: Option<JoinHandle<()>>,
}

impl AnalysisLaneWorker {
    pub fn diagnostics(&self) -> LaneDiagnostics {
        self.state.diagnostics()
    }

    pub fn wait(mut self) -> Result<(), LaneError> {
        self.join
            .take()
            .expect("analysis lane worker already joined")
            .join()
            .map_err(|_| LaneError::WorkerPanicked)
    }
}

impl Drop for AnalysisLaneWorker {
    fn drop(&mut self) {
        self.state.shutdown.store(true, Ordering::Release);
        self.input.available.notify_all();
        self.output.available.notify_all();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn analysis_execution_lane<T, P>(
    input_capacity: usize,
    output_capacity: usize,
    policy: LanePolicy,
    processor: P,
) -> Result<(AnalysisLaneSender, AnalysisLaneReceiver<T>, AnalysisLaneWorker), LaneError>
where
    T: Send + 'static,
    P: Fn(AnalysisBlock) -> T + Send + 'static,
{
    if input_capacity == 0 || output_capacity == 0 {
        return Err(LaneError::InvalidCapacity);
    }
    let input = Arc::new(Mailbox::new(input_capacity));
    let output = Arc::new(Mailbox::new(output_capacity));
    let input_signal = Arc::new(MailboxErased { available: Arc::clone(&input.available) });
    let output_signal = Arc::new(MailboxErased { available: Arc::clone(&output.available) });
    let state = Arc::new(LaneState {
        shutdown: AtomicBool::new(false),
        sender_connected: AtomicBool::new(true),
        receiver_connected: AtomicBool::new(true),
        incomplete: AtomicBool::new(false),
        input_depth: AtomicUsize::new(0),
        input_high_water: AtomicUsize::new(0),
        output_depth: AtomicUsize::new(0),
        output_high_water: AtomicUsize::new(0),
        visual_drops: AtomicU64::new(0),
        stale_visual_inputs: AtomicU64::new(0),
        continuous_gaps: AtomicU64::new(0),
        processor_panics: AtomicU64::new(0),
    });
    let worker_input = Arc::clone(&input);
    let worker_output = Arc::clone(&output);
    let worker_state = Arc::clone(&state);
    let join = thread::Builder::new()
        .name(
            match policy {
                LanePolicy::Continuous => "pulseseek-analysis-continuous",
                LanePolicy::LatestOnly => "pulseseek-analysis-visual",
            }
            .into(),
        )
        .spawn(move || run_lane(worker_input, worker_output, policy, processor, &worker_state))
        .map_err(|_| LaneError::WorkerStartFailed)?;
    Ok((
        AnalysisLaneSender {
            input,
            output: Arc::clone(&output_signal),
            policy,
            state: Arc::clone(&state),
        },
        AnalysisLaneReceiver {
            output,
            input: Arc::clone(&input_signal),
            state: Arc::clone(&state),
        },
        AnalysisLaneWorker { state, input: input_signal, output: output_signal, join: Some(join) },
    ))
}

fn run_lane<T, P>(
    input: Arc<Mailbox<AnalysisBlock>>,
    output: Arc<Mailbox<LaneOutput<T>>>,
    policy: LanePolicy,
    processor: P,
    state: &LaneState,
) where
    T: Send + 'static,
    P: Fn(AnalysisBlock) -> T,
{
    let mut previous_sequence = None;
    while let Some(block) = receive_input(&input, policy, state) {
        let sequence_gap = previous_sequence
            .is_some_and(|sequence: u64| block.sequence() != sequence.wrapping_add(1));
        previous_sequence = Some(block.sequence());
        if policy == LanePolicy::Continuous && (block.discontinuity() || sequence_gap) {
            state.continuous_gaps.fetch_add(1, Ordering::Relaxed);
            state.incomplete.store(true, Ordering::Release);
        }
        let processed = panic::catch_unwind(AssertUnwindSafe(|| processor(block)));
        let value = match processed {
            Ok(value) => value,
            Err(_) => {
                state.processor_panics.fetch_add(1, Ordering::Relaxed);
                state.incomplete.store(true, Ordering::Release);
                continue;
            },
        };
        let validity = match policy {
            LanePolicy::LatestOnly => LaneValidity::Measured,
            LanePolicy::Continuous if state.incomplete.load(Ordering::Acquire) => {
                LaneValidity::Incomplete
            },
            LanePolicy::Continuous => LaneValidity::Complete,
        };
        publish_output(&output, LaneOutput { value, validity }, policy, state);
    }
    let mut queue = input.queue.lock().unwrap_or_else(|error| error.into_inner());
    queue.clear();
    state.input_depth.store(0, Ordering::Release);
}

fn receive_input(
    input: &Mailbox<AnalysisBlock>,
    policy: LanePolicy,
    state: &LaneState,
) -> Option<AnalysisBlock> {
    let mut queue = input.queue.lock().unwrap_or_else(|error| error.into_inner());
    loop {
        if state.shutdown.load(Ordering::Acquire)
            || !state.receiver_connected.load(Ordering::Acquire)
        {
            return None;
        }
        if let Some(mut block) = queue.pop_front() {
            if policy == LanePolicy::LatestOnly {
                while let Some(newer) = queue.pop_front() {
                    state.stale_visual_inputs.fetch_add(1, Ordering::Relaxed);
                    block = newer;
                }
            }
            state.input_depth.store(queue.len(), Ordering::Release);
            return Some(block);
        }
        if !state.sender_connected.load(Ordering::Acquire) {
            return None;
        }
        queue = input.available.wait(queue).unwrap_or_else(|error| error.into_inner());
    }
}

fn publish_output<T>(
    output: &Mailbox<LaneOutput<T>>,
    value: LaneOutput<T>,
    policy: LanePolicy,
    state: &LaneState,
) {
    if !state.receiver_connected.load(Ordering::Acquire) {
        return;
    }
    let mut queue = output.queue.lock().unwrap_or_else(|error| error.into_inner());
    if queue.len() == output.capacity {
        match policy {
            LanePolicy::LatestOnly => {
                queue.pop_front();
                queue.push_back(value);
                state.visual_drops.fetch_add(1, Ordering::Relaxed);
                output.available.notify_one();
            },
            LanePolicy::Continuous => {
                state.continuous_gaps.fetch_add(1, Ordering::Relaxed);
                state.incomplete.store(true, Ordering::Release);
            },
        }
        return;
    }
    queue.push_back(value);
    let depth = queue.len();
    state.output_depth.store(depth, Ordering::Release);
    state.output_high_water.fetch_max(depth, Ordering::Relaxed);
    output.available.notify_one();
}
