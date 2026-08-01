use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait};

use pulseseek_domain::audio_output::{AudioOutputError, DeviceId, DeviceInfo, StreamState};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_playback::PlaybackControl;

pub(crate) const STATE_PLAYING: u8 = 0;
pub(crate) const STATE_PAUSED: u8 = 1;
pub(crate) const STATE_STOPPED: u8 = 2;
pub(crate) const STATE_LOST_PAUSED: u8 = 3;
pub(crate) const STATE_LOST_STOPPED: u8 = 4;

#[derive(Clone)]
pub(crate) struct StreamStatus {
    state: Arc<AtomicU8>,
}

impl StreamStatus {
    pub(crate) fn new() -> Self {
        Self { state: Arc::new(AtomicU8::new(STATE_STOPPED)) }
    }

    pub(crate) fn set_playing(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            if self.is_lost_state(current) {
                return;
            }
            if self
                .state
                .compare_exchange(current, STATE_PLAYING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    pub(crate) fn set_paused(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            if current == STATE_LOST_PAUSED || current == STATE_LOST_STOPPED {
                return;
            }
            if self
                .state
                .compare_exchange(current, STATE_PAUSED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    pub(crate) fn set_stopped(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let stopped =
                if self.is_lost_state(current) { STATE_LOST_STOPPED } else { STATE_STOPPED };
            if self
                .state
                .compare_exchange(current, stopped, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    fn is_lost_state(&self, state: u8) -> bool {
        state == STATE_LOST_PAUSED || state == STATE_LOST_STOPPED
    }

    pub(crate) fn reset_after_open(&self) {
        self.state.store(STATE_STOPPED, Ordering::Release);
    }

    pub(crate) fn mark_device_lost(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let lost = if current == STATE_STOPPED || current == STATE_LOST_STOPPED {
                STATE_LOST_STOPPED
            } else {
                STATE_LOST_PAUSED
            };
            if self
                .state
                .compare_exchange(current, lost, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    pub(crate) fn is_device_lost(&self) -> bool {
        self.is_lost_state(self.state.load(Ordering::Acquire))
    }

    pub(crate) fn state(&self) -> StreamState {
        match self.state.load(Ordering::Acquire) {
            STATE_PLAYING => StreamState::Playing,
            STATE_PAUSED | STATE_LOST_PAUSED => StreamState::Paused,
            _ => StreamState::Stopped,
        }
    }
}

pub(crate) enum StreamCommand {
    Play(SyncSender<Result<(), AudioOutputError>>),
    Pause(SyncSender<Result<(), AudioOutputError>>),
    Stop(SyncSender<Result<(), AudioOutputError>>),
}

pub(crate) struct StreamControl {
    pub(crate) commands: Sender<StreamCommand>,
    pub(crate) error: Arc<Mutex<Option<String>>>,
    pub(crate) join: Option<JoinHandle<()>>,
}

impl StreamControl {
    pub(crate) fn stream_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|error| error.clone())
    }

    fn request(
        &self,
        command: impl FnOnce(SyncSender<Result<(), AudioOutputError>>) -> StreamCommand,
    ) -> Result<(), AudioOutputError> {
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        self.commands.send(command(response_tx)).map_err(|_| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("audio stream thread stopped"),
            )
        })?;
        response_rx.recv().map_err(|_| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("audio stream thread stopped"),
            )
        })?
    }

    pub(crate) fn play(&self) -> Result<(), AudioOutputError> {
        self.request(StreamCommand::Play)
    }

    pub(crate) fn pause(&self) -> Result<(), AudioOutputError> {
        self.request(StreamCommand::Pause)
    }

    pub(crate) fn stop(mut self) -> Result<(), AudioOutputError> {
        let result = self.request(StreamCommand::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        result
    }
}

impl Drop for StreamControl {
    fn drop(&mut self) {
        let (response_tx, _response_rx) = mpsc::sync_channel(1);
        let _ = self.commands.send(StreamCommand::Stop(response_tx));
    }
}

#[derive(Clone)]
pub(crate) struct StreamCallbackContext {
    pub(crate) status: StreamStatus,
    pub(crate) playback_control: PlaybackControl,
    pub(crate) volume_gain: Arc<AtomicU32>,
    pub(crate) stream_error: Arc<Mutex<Option<String>>>,
}

pub(crate) fn find_device(host: &cpal::Host, device_id: &DeviceId) -> Option<cpal::Device> {
    if let Some(default) = host.default_output_device() {
        if let Ok(id) = default.id() {
            if DeviceId::new(id.to_string()) == *device_id {
                return Some(default);
            }
        }
    }

    if let Ok(outputs) = host.output_devices() {
        for device in outputs {
            if let Ok(id) = device.id() {
                if DeviceId::new(id.to_string()) == *device_id {
                    return Some(device);
                }
            }
        }
    }

    None
}

pub(crate) fn device_info(device: &cpal::Device) -> Result<DeviceInfo, AudioOutputError> {
    let name =
        device.description().map(|description| description.name().to_string()).map_err(|e| {
            AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
        })?;

    let id = DeviceId::new(
        device
            .id()
            .map_err(|e| {
                AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
            })?
            .to_string(),
    );

    let mut max_channels: u16 = 0;
    let mut sample_rates: Vec<u32> = Vec::new();
    let mut seen_rates: HashSet<u32> = HashSet::new();

    if let Ok(configs) = device.supported_output_configs() {
        for cfg in configs {
            let ch = cfg.channels();
            if ch > max_channels {
                max_channels = ch;
            }
            let min_rate = cfg.min_sample_rate();
            let max_rate = cfg.max_sample_rate();
            for rate in &[min_rate, max_rate] {
                if seen_rates.insert(*rate) {
                    sample_rates.push(*rate);
                }
            }
        }
    }

    sample_rates.sort();

    Ok(DeviceInfo { id, name, max_channels, sample_rates })
}
