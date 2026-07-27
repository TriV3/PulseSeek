use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use pulseseek_domain::audio_output::{
    AudioOutput, AudioOutputError, DeviceId, DeviceInfo, StreamState,
};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_domain::playback::volume::{Gain, Volume};
use pulseseek_playback::PlaybackConsumer;

/// A cpal-based audio output adapter.
pub struct CpalAudioOutput {
    current_device: Option<DeviceId>,
    active_device: Option<cpal::Device>,
    stream: Option<StreamControl>,
    state: StreamState,
    #[allow(dead_code)]
    volume: Volume,
    volume_gain: Arc<AtomicU32>,
    #[allow(dead_code)]
    device_lost: bool,
}

enum StreamCommand {
    Play(SyncSender<Result<(), AudioOutputError>>),
    Pause(SyncSender<Result<(), AudioOutputError>>),
    Stop(SyncSender<Result<(), AudioOutputError>>),
}

struct StreamControl {
    commands: Sender<StreamCommand>,
    error: Arc<Mutex<Option<String>>>,
    join: Option<JoinHandle<()>>,
}

impl StreamControl {
    fn stream_error(&self) -> Option<String> {
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

    fn play(&self) -> Result<(), AudioOutputError> {
        self.request(StreamCommand::Play)
    }

    fn pause(&self) -> Result<(), AudioOutputError> {
        self.request(StreamCommand::Pause)
    }

    fn stop(mut self) -> Result<(), AudioOutputError> {
        let result = self.request(StreamCommand::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        result
    }
}

impl Drop for StreamControl {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            let (response_tx, _response_rx) = mpsc::sync_channel(1);
            let _ = self.commands.send(StreamCommand::Stop(response_tx));
            let _ = join.join();
        }
    }
}

impl CpalAudioOutput {
    /// Creates a new cpal audio output adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the latest asynchronous stream error, if one occurred.
    pub fn stream_error(&self) -> Option<String> {
        self.stream.as_ref().and_then(StreamControl::stream_error)
    }
}

impl Default for CpalAudioOutput {
    fn default() -> Self {
        Self {
            current_device: None,
            active_device: None,
            stream: None,
            state: StreamState::Stopped,
            volume: Volume::new(Gain::new(1.0)),
            volume_gain: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            device_lost: false,
        }
    }
}

impl CpalAudioOutput {
    fn find_device(host: &cpal::Host, device_id: &DeviceId) -> Option<cpal::Device> {
        // Check default device first.
        if let Some(default) = host.default_output_device() {
            if let Ok(name) = default.name() {
                if DeviceId::new(name) == *device_id {
                    return Some(default);
                }
            }
        }

        // Search all output devices.
        if let Ok(outputs) = host.output_devices() {
            for device in outputs {
                if let Ok(name) = device.name() {
                    if DeviceId::new(name) == *device_id {
                        return Some(device);
                    }
                }
            }
        }

        None
    }

    /// Maps a cpal device to a domain DeviceInfo.
    fn device_info(device: &cpal::Device) -> Result<DeviceInfo, AudioOutputError> {
        let name = device.name().map_err(|e| {
            AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
        })?;

        let id = DeviceId::new(name.clone());

        let mut max_channels: u16 = 0;
        let mut sample_rates: Vec<u32> = Vec::new();
        let mut seen_rates: HashSet<u32> = HashSet::new();

        if let Ok(configs) = device.supported_output_configs() {
            for cfg in configs {
                let ch = cfg.channels();
                if ch > max_channels {
                    max_channels = ch;
                }
                let min_rate = cfg.min_sample_rate().0;
                let max_rate = cfg.max_sample_rate().0;
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

    /// Builds an output stream using decoder interleaved channel count.
    pub fn open_stream(
        &mut self,
        consumer: PlaybackConsumer,
        source_channels: usize,
    ) -> Result<(), AudioOutputError> {
        if source_channels == 0 {
            return Err(AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("source channel count must be greater than zero"),
            ));
        }
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        let device = self.active_device.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output device is not open"),
            )
        })?;
        let supported = device.default_output_config().map_err(|e| {
            AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
        })?;
        let device = device.clone();
        let config: cpal::StreamConfig = supported.clone().into();
        let output_channels = config.channels as usize;
        if output_channels == 0 || output_channels > 32 {
            return Err(AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("unsupported output channel count"),
            ));
        }
        let sample_format = supported.sample_format();
        let stream_error = Arc::new(Mutex::new(None));
        let callback_error = Arc::clone(&stream_error);
        let volume_gain = Arc::clone(&self.volume_gain);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (command_tx, command_rx) = mpsc::channel();
        let join = std::thread::spawn(move || {
            let stream = match sample_format {
                cpal::SampleFormat::F32 => Self::build_stream::<f32>(
                    &device,
                    &config,
                    consumer,
                    source_channels,
                    output_channels,
                    Arc::clone(&volume_gain),
                    callback_error.clone(),
                ),
                cpal::SampleFormat::I16 => Self::build_stream::<i16>(
                    &device,
                    &config,
                    consumer,
                    source_channels,
                    output_channels,
                    Arc::clone(&volume_gain),
                    callback_error.clone(),
                ),
                cpal::SampleFormat::U16 => Self::build_stream::<u16>(
                    &device,
                    &config,
                    consumer,
                    source_channels,
                    output_channels,
                    Arc::clone(&volume_gain),
                    callback_error.clone(),
                ),
                format => Err(AudioOutputError::new(
                    DiagnosticContext::new(DiagnosticCode::AudioOutput),
                    std::io::Error::other(format!("unsupported output sample format: {format:?}")),
                )),
            };
            let stream = match stream {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                },
            };
            if ready_tx.send(Ok(())).is_err() {
                return;
            }
            for command in command_rx {
                match command {
                    StreamCommand::Play(response) => {
                        let _ = response.send(stream.play().map_err(|e| {
                            AudioOutputError::new(
                                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                                e,
                            )
                        }));
                    },
                    StreamCommand::Pause(response) => {
                        let _ = response.send(stream.pause().map_err(|e| {
                            AudioOutputError::new(
                                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                                e,
                            )
                        }));
                    },
                    StreamCommand::Stop(response) => {
                        let _ = response.send(Ok(()));
                        break;
                    },
                }
            }
        });

        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.stream = Some(StreamControl {
                    commands: command_tx,
                    error: stream_error,
                    join: Some(join),
                });
                Ok(())
            },
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            },
            Err(_) => {
                let _ = join.join();
                Err(AudioOutputError::new(
                    DiagnosticContext::new(DiagnosticCode::AudioOutput),
                    std::io::Error::other("audio stream thread failed to initialize"),
                ))
            },
        }
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut consumer: PlaybackConsumer,
        source_channels: usize,
        output_channels: usize,
        volume_gain: Arc<AtomicU32>,
        stream_error: Arc<Mutex<Option<String>>>,
    ) -> Result<cpal::Stream, AudioOutputError>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _| {
                    let mut mapped = [0.0f32; 32];
                    let gain = f32::from_bits(volume_gain.load(Ordering::Relaxed));
                    for frame in data.chunks_mut(output_channels) {
                        if output_channels > mapped.len() {
                            for output in frame {
                                *output = T::from_sample(0.0);
                            }
                            continue;
                        }
                        consumer.consume_channels_with_volume(
                            &mut mapped[..output_channels],
                            source_channels,
                            output_channels,
                            gain,
                        );
                        for (output, sample) in frame.iter_mut().zip(&mapped[..output_channels]) {
                            *output = T::from_sample(*sample);
                        }
                    }
                },
                move |error| {
                    if let Ok(mut slot) = stream_error.lock() {
                        *slot = Some(error.to_string());
                    }
                },
                None,
            )
            .map_err(|e| {
                AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), e)
            })?;
        Ok(stream)
    }
}

impl AudioOutput for CpalAudioOutput {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioOutputError> {
        let host = cpal::default_host();
        let mut devices: Vec<DeviceInfo> = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        if let Some(default) = host.default_output_device() {
            if let Ok(info) = Self::device_info(&default) {
                seen_ids.insert(info.id.as_str().to_string());
                devices.push(info);
            }
        }

        if let Ok(outputs) = host.output_devices() {
            for device in outputs {
                if let Ok(info) = Self::device_info(&device) {
                    if seen_ids.insert(info.id.as_str().to_string()) {
                        devices.push(info);
                    }
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, device_id: &DeviceId) -> Result<(), AudioOutputError> {
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        let host = cpal::default_host();

        // Try to find the requested device by ID.
        let device = Self::find_device(&host, device_id)
            .or_else(|| {
                // Fallback: try default device.
                host.default_output_device()
            })
            .ok_or_else(|| {
                AudioOutputError::new(
                    DiagnosticContext::new(DiagnosticCode::AudioOutput),
                    std::io::Error::other("no output device available"),
                )
            })?;

        self.current_device = if let Ok(name) = device.name() {
            Some(DeviceId::new(name))
        } else {
            Some(device_id.clone())
        };
        self.active_device = Some(device);
        self.state = StreamState::Stopped;
        Ok(())
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        let stream = self.stream.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output stream is not open"),
            )
        })?;
        stream.play()?;
        self.state = StreamState::Playing;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        let stream = self.stream.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output stream is not open"),
            )
        })?;
        stream.pause()?;
        self.state = StreamState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioOutputError> {
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        self.state = StreamState::Stopped;
        Ok(())
    }

    fn set_volume(&mut self, volume: Volume) -> Result<(), AudioOutputError> {
        self.volume = volume;
        self.volume_gain.store((volume.effective_gain() as f32).to_bits(), Ordering::Release);
        Ok(())
    }

    fn is_device_lost(&self) -> bool {
        self.device_lost
    }

    fn current_device(&self) -> Option<DeviceId> {
        self.current_device.clone()
    }

    fn state(&self) -> StreamState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_name_to_id_uses_name() {
        let id = DeviceId::new("Test Speaker");
        assert_eq!(id.as_str(), "Test Speaker");
    }

    #[test]
    fn enumerate_returns_devices_with_id_and_name() {
        let output = CpalAudioOutput::new();
        let devices = output.list_devices().expect("list_devices should succeed");
        if devices.is_empty() {
            return;
        }

        for d in &devices {
            assert!(!d.id.as_str().is_empty(), "device id should not be empty");
            assert!(!d.name.is_empty(), "device name should not be empty");
        }
    }

    #[test]
    fn enumerate_includes_default_device() {
        let output = CpalAudioOutput::new();
        let devices = output.list_devices().expect("list_devices should succeed");

        let host = cpal::default_host();
        if let Some(default) = host.default_output_device() {
            if let Ok(name) = default.name() {
                let found = devices.iter().any(|d| d.name == name);
                assert!(found, "default device '{}' should be in device list", name);
            }
        }
    }

    #[test]
    fn open_known_device_sets_current() {
        let mut output = CpalAudioOutput::new();
        let devices = output.list_devices().expect("list_devices should succeed");
        if devices.is_empty() {
            return;
        }

        let id = devices[0].id.clone();
        output.open(&id).expect("open should succeed for known device");
        assert_eq!(output.current_device(), Some(id));
    }

    #[test]
    fn open_unknown_device_falls_back_to_default() {
        let mut output = CpalAudioOutput::new();
        let unknown = DeviceId::new("__nonexistent_device__");

        let host = cpal::default_host();
        if host.default_output_device().is_some() {
            // Fallback should succeed.
            output.open(&unknown).expect("open should fall back to default");
            assert!(output.current_device().is_some(), "should have a device after fallback");
        } else {
            // No default available — open should fail.
            assert!(output.open(&unknown).is_err(), "open should fail without fallback");
        }
    }

    #[test]
    fn play_requires_an_open_stream() {
        let mut output = CpalAudioOutput::new();

        assert!(output.play().is_err());
        assert_eq!(output.state(), StreamState::Stopped);
    }

    #[test]
    fn stop_without_stream_is_idempotent() {
        let mut output = CpalAudioOutput::new();

        output.stop().expect("stop should be safe before stream creation");
        output.stop().expect("stop should remain idempotent");
        assert_eq!(output.state(), StreamState::Stopped);
    }

    #[test]
    fn set_volume_updates_callback_gain_without_creating_stream() {
        let mut output = CpalAudioOutput::new();

        output.set_volume(Volume::new(Gain::new(0.25))).expect("volume update should succeed");

        assert_eq!(f32::from_bits(output.volume_gain.load(Ordering::Relaxed)), 0.25);
        assert!(output.stream.is_none(), "volume update must not create a stream");
    }
}
