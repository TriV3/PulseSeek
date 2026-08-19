use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use pulseseek_domain::analysis::{
    AnalysisBlock, AudioFormat, MeasurementPoint, SessionId, SourceId, SourceKind,
};
use pulseseek_playback::{
    analysis_execution_lane, LaneError, LanePolicy, LaneValidity, SubmissionResult,
};

fn block(sequence: u64, discontinuity: bool) -> AnalysisBlock {
    AnalysisBlock::new(
        SourceId::new("player"),
        SessionId::new("session-1"),
        SourceKind::Playback,
        MeasurementPoint::Source,
        AudioFormat::mono(48_000).unwrap(),
        sequence,
        1,
        sequence,
        discontinuity,
        vec![sequence as f32],
    )
    .unwrap()
}

#[test]
fn visual_lane_discards_stale_work_without_blocking_producer() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (sender, receiver, worker) =
        analysis_execution_lane(2, 2, LanePolicy::LatestOnly, move |b| {
            if b.sequence() == 0 {
                started_sender.send(()).unwrap();
                release_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            b.sequence()
        })
        .unwrap();

    assert_eq!(sender.try_submit(block(0, false)), SubmissionResult::Accepted);
    started_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(sender.try_submit(block(1, false)), SubmissionResult::Accepted);
    assert_eq!(sender.try_submit(block(2, false)), SubmissionResult::Accepted);
    assert_eq!(sender.try_submit(block(3, false)), SubmissionResult::DroppedVisual);
    release_sender.send(()).unwrap();

    let first = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    let latest = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(first.value, 0);
    assert_eq!(latest.value, 3);
    assert_eq!(latest.validity, LaneValidity::Measured);
    assert_eq!(sender.diagnostics().visual_drops, 1);
    assert_eq!(sender.diagnostics().stale_visual_inputs, 1);

    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn continuous_saturation_marks_next_result_incomplete() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (sender, receiver, worker) =
        analysis_execution_lane(1, 2, LanePolicy::Continuous, move |b| {
            if b.sequence() == 0 {
                started_sender.send(()).unwrap();
                release_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            b.sequence()
        })
        .unwrap();

    assert_eq!(sender.try_submit(block(0, false)), SubmissionResult::Accepted);
    started_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(sender.try_submit(block(1, false)), SubmissionResult::Accepted);
    assert_eq!(sender.try_submit(block(2, false)), SubmissionResult::ContinuousGap);
    release_sender.send(()).unwrap();
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap().validity,
        LaneValidity::Incomplete
    );
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap().validity,
        LaneValidity::Incomplete
    );
    assert_eq!(sender.try_submit(block(3, false)), SubmissionResult::Accepted);
    let after_gap = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(after_gap.value, 3);
    assert_eq!(after_gap.validity, LaneValidity::Incomplete);
    assert_eq!(sender.diagnostics().continuous_gaps, 2);

    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn discontinuity_and_sequence_gap_mark_continuous_results_incomplete() {
    let (sender, receiver, worker) =
        analysis_execution_lane(3, 3, LanePolicy::Continuous, |b| b.sequence()).unwrap();

    assert_eq!(sender.try_submit(block(4, false)), SubmissionResult::Accepted);
    assert_eq!(sender.try_submit(block(6, false)), SubmissionResult::Accepted);
    assert_eq!(sender.try_submit(block(7, true)), SubmissionResult::Accepted);

    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap().validity,
        LaneValidity::Complete
    );
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap().validity,
        LaneValidity::Incomplete
    );
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap().validity,
        LaneValidity::Incomplete
    );
    assert_eq!(sender.diagnostics().continuous_gaps, 2);

    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn bounded_backpressure_returns_immediately_and_reports_depth() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (sender, receiver, worker) =
        analysis_execution_lane(1, 1, LanePolicy::LatestOnly, move |b| {
            if b.sequence() == 0 {
                started_sender.send(()).unwrap();
                release_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            b.sequence()
        })
        .unwrap();

    let sender = Arc::new(sender);
    sender.try_submit(block(0, false));
    started_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    sender.try_submit(block(1, false));
    let (completed_sender, completed_receiver) = mpsc::channel();
    thread::scope(|scope| {
        let observed_sender = Arc::clone(&sender);
        scope.spawn(move || {
            for _ in 0..10_000 {
                let diagnostics = observed_sender.diagnostics();
                assert!(diagnostics.input_depth <= 1);
                assert!(diagnostics.output_depth <= 1);
            }
        });
        let submitting_sender = Arc::clone(&sender);
        scope.spawn(move || {
            for sequence in 2..10_002 {
                submitting_sender.try_submit(block(sequence, false));
            }
            completed_sender.send(()).unwrap();
        });
        completed_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    release_sender.send(()).unwrap();
    let diagnostics = sender.diagnostics();
    assert!(diagnostics.visual_drops > 0);
    assert_eq!(diagnostics.input_high_water, 1);
    assert!(diagnostics.input_depth <= 1);

    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn dropping_last_receiver_stops_idle_worker() {
    let (_sender, receiver, worker) =
        analysis_execution_lane(1, 1, LanePolicy::Continuous, |b| b.sequence()).unwrap();

    drop(receiver);

    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn dropping_last_sender_stops_idle_worker_and_clears_input_depth() {
    let (sender, receiver, worker) =
        analysis_execution_lane(1, 1, LanePolicy::Continuous, |b| b.sequence()).unwrap();
    drop(sender);

    assert_eq!(worker.wait(), Ok(()));
    assert_eq!(receiver.diagnostics().input_depth, 0);
    assert!(matches!(
        receiver.recv_timeout(Duration::from_millis(10)),
        Err(pulseseek_playback::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn processor_panic_is_localized_and_sibling_lane_remains_healthy() {
    let panic_once = Arc::new(AtomicBool::new(true));
    let panic_flag = Arc::clone(&panic_once);
    let (sender, receiver, worker) =
        analysis_execution_lane(2, 2, LanePolicy::Continuous, move |b| {
            if panic_flag.swap(false, Ordering::AcqRel) {
                panic!("isolated processor failure");
            }
            b.sequence()
        })
        .unwrap();
    let (sibling_sender, sibling_receiver, sibling_worker) =
        analysis_execution_lane(1, 1, LanePolicy::LatestOnly, |b| b.sequence()).unwrap();

    sender.try_submit(block(0, false));
    sender.try_submit(block(1, false));
    sibling_sender.try_submit(block(9, false));

    let recovered = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(recovered.value, 1);
    assert_eq!(recovered.validity, LaneValidity::Incomplete);
    assert_eq!(sender.diagnostics().processor_panics, 1);
    assert_eq!(sibling_receiver.recv_timeout(Duration::from_secs(1)).unwrap().value, 9);

    drop(receiver);
    drop(sibling_receiver);
    assert_eq!(worker.wait(), Ok(()));
    assert_eq!(sibling_worker.wait(), Ok(()));
}

#[test]
fn capacities_must_be_positive() {
    let zero_input = analysis_execution_lane(0, 1, LanePolicy::Continuous, |b| b.sequence());
    let zero_output = analysis_execution_lane(1, 0, LanePolicy::Continuous, |b| b.sequence());
    let positive = analysis_execution_lane(1, 1, LanePolicy::Continuous, |b| b.sequence());

    assert!(matches!(zero_input, Err(LaneError::InvalidCapacity)));
    assert!(matches!(zero_output, Err(LaneError::InvalidCapacity)));
    assert!(positive.is_ok());
}

#[test]
fn explicit_shutdown_rejects_later_submissions() {
    let (sender, receiver, worker) =
        analysis_execution_lane(1, 1, LanePolicy::Continuous, |b| b.sequence()).unwrap();

    sender.shutdown();

    assert_eq!(sender.try_submit(block(0, false)), SubmissionResult::Shutdown);
    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn dropping_receiver_makes_disconnection_observable() {
    let (sender, receiver, worker) =
        analysis_execution_lane(1, 1, LanePolicy::Continuous, |b| b.sequence()).unwrap();

    drop(receiver);

    assert_eq!(sender.try_submit(block(0, false)), SubmissionResult::ReceiverGone);
    assert_eq!(worker.wait(), Ok(()));
}

#[test]
fn output_saturation_is_counted_and_depth_remains_bounded() {
    let (sender, receiver, worker) =
        analysis_execution_lane(3, 1, LanePolicy::LatestOnly, |b| b.sequence()).unwrap();

    assert_eq!(sender.try_submit(block(0, false)), SubmissionResult::Accepted);
    let deadline = Instant::now() + Duration::from_secs(1);
    while receiver.diagnostics().output_depth == 0 && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(receiver.diagnostics().output_depth, 1);
    assert_eq!(sender.try_submit(block(1, false)), SubmissionResult::Accepted);
    let deadline = Instant::now() + Duration::from_secs(1);
    while sender.diagnostics().visual_drops == 0 && Instant::now() < deadline {
        thread::yield_now();
    }

    let diagnostics = sender.diagnostics();
    assert_eq!(diagnostics.visual_drops, 1);
    assert_eq!(diagnostics.output_depth, 1);
    assert_eq!(diagnostics.output_high_water, 1);
    assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap().value, 1);

    drop(receiver);
    assert_eq!(worker.wait(), Ok(()));
}
