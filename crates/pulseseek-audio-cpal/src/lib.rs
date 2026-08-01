mod stream;

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use pulseseek_domain::audio_output::{
    AudioOutput, AudioOutputError, DeviceId, DeviceInfo, StreamState,
};
use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext};
use pulseseek_domain::playback::volume::{Gain, Volume};
use pulseseek_playback::{PlaybackConsumer, PlaybackControl};

/// A cpal-based audio output adapter.
pub struct CpalAudioOutput {
    current_device: Option<DeviceId>,
    active_device: Option<cpal::Device>,
    pub(crate) stream: Option<StreamControl>,
    playback_control: Option<PlaybackControl>,
    pub(crate) status: StreamStatus,
    #[allow(dead_code)]
    volume: Volume,
    pub(crate) volume_gain: Arc<AtomicU32>,
}

use self::stream::*;

impl CpalAudioOutput {
    /// Creates a new cpal audio output adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the latest asynchronous stream error, if one occurred.
    pub fn stream_error(&self) -> Option<String> {
        self.stream.as_ref().and_then(StreamControl::stream_error)
    }

    /// Returns active device's default output sample rate.
    ///
    /// Playback workers use this value as their resampling target.
    pub fn output_sample_rate(&self) -> Result<u32, AudioOutputError> {
        let device = self.active_device.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output device is not open"),
            )
        })?;
        Ok(device
            .default_output_config()
            .map_err(|error| {
                AudioOutputError::new(DiagnosticContext::new(DiagnosticCode::AudioOutput), error)
            })?
            .sample_rate())
    }
}

impl Default for CpalAudioOutput {
    fn default() -> Self {
        Self {
            current_device: None,
            active_device: None,
            stream: None,
            playback_control: None,
            status: StreamStatus::new(),
            volume: Volume::new(Gain::new(1.0)),
            volume_gain: Arc::new(AtomicU32::new(1.0f32.to_bits())),
        }
    }
}

impl CpalAudioOutput {
    /// Builds an output stream using the device's native default configuration.
    /// The playback worker is responsible for converting source samples to
    /// this rate before they reach the real-time callback.
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
        if let Some(control) = &self.playback_control {
            control.stop();
        }
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        self.playback_control = None;
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
        let stream_status = self.status.clone();
        let volume_gain = Arc::clone(&self.volume_gain);
        let playback_control = consumer.control();
        let callback_control = playback_control.clone();
        let callback_context = StreamCallbackContext {
            status: stream_status,
            playback_control: callback_control,
            volume_gain,
            stream_error: Arc::clone(&stream_error),
        };
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
                    callback_context.clone(),
                ),
                cpal::SampleFormat::I16 => Self::build_stream::<i16>(
                    &device,
                    &config,
                    consumer,
                    source_channels,
                    output_channels,
                    callback_context.clone(),
                ),
                cpal::SampleFormat::U16 => Self::build_stream::<u16>(
                    &device,
                    &config,
                    consumer,
                    source_channels,
                    output_channels,
                    callback_context,
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
                self.playback_control = Some(playback_control);
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
        context: StreamCallbackContext,
    ) -> Result<cpal::Stream, AudioOutputError>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let data_volume_gain = Arc::clone(&context.volume_gain);
        let error_status = context.status;
        let error_control = context.playback_control;
        let stream_error = context.stream_error;
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _| {
                    let mut mapped = [0.0f32; 32];
                    let gain = f32::from_bits(data_volume_gain.load(Ordering::Relaxed));
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
                    error_status.mark_device_lost();
                    error_control.pause();
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
            if let Ok(info) = stream::device_info(&default) {
                seen_ids.insert(info.id.as_str().to_string());
                devices.push(info);
            }
        }

        if let Ok(outputs) = host.output_devices() {
            for device in outputs {
                if let Ok(info) = stream::device_info(&device) {
                    if seen_ids.insert(info.id.as_str().to_string()) {
                        devices.push(info);
                    }
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, device_id: &DeviceId) -> Result<(), AudioOutputError> {
        if let Some(control) = &self.playback_control {
            control.stop();
        }
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        self.playback_control = None;
        let host = cpal::default_host();

        // Try to find the requested device by ID.
        let device = stream::find_device(&host, device_id)
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

        self.current_device = if let Ok(id) = device.id() {
            Some(DeviceId::new(id.to_string()))
        } else {
            Some(device_id.clone())
        };
        self.active_device = Some(device);
        self.status.reset_after_open();
        Ok(())
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        if self.status.is_device_lost() {
            return Err(AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output device was lost; open a device before playing"),
            ));
        }
        let stream = self.stream.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output stream is not open"),
            )
        })?;
        stream.play()?;
        if let Some(control) = &self.playback_control {
            control.resume();
        }
        self.status.set_playing();
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        if self.status.is_device_lost() {
            self.status.set_paused();
            if let Some(control) = &self.playback_control {
                control.pause();
            }
            return Ok(());
        }
        let stream = self.stream.as_ref().ok_or_else(|| {
            AudioOutputError::new(
                DiagnosticContext::new(DiagnosticCode::AudioOutput),
                std::io::Error::other("output stream is not open"),
            )
        })?;
        if let Some(control) = &self.playback_control {
            control.pause();
        }
        if let Err(error) = stream.pause() {
            if let Some(control) = &self.playback_control {
                control.resume();
            }
            return Err(error);
        }
        self.status.set_paused();
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioOutputError> {
        if let Some(control) = &self.playback_control {
            control.stop();
        }
        if let Some(stream) = self.stream.take() {
            stream.stop()?;
        }
        self.playback_control = None;
        self.status.set_stopped();
        Ok(())
    }

    fn set_volume(&mut self, volume: Volume) -> Result<(), AudioOutputError> {
        self.volume = volume;
        self.volume_gain.store((volume.effective_gain() as f32).to_bits(), Ordering::Release);
        Ok(())
    }

    fn is_device_lost(&self) -> bool {
        self.status.is_device_lost()
    }

    fn current_device(&self) -> Option<DeviceId> {
        self.current_device.clone()
    }

    fn state(&self) -> StreamState {
        self.status.state()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
