use pulseseek_domain::decoder::{Decoder, ProbeResult, StreamMetadata};
use pulseseek_domain::playback::position::{Duration, Position, SeekTarget};

/// A fake decoder that plays back pre-recorded PCM data.
struct FakeDecoder {
    data: Vec<f32>,
    position: usize,
    expected_metadata: StreamMetadata,
}

impl FakeDecoder {
    fn new(data: Vec<f32>, sample_rate: u32, channels: u16, duration_ms: u64) -> Self {
        Self {
            data,
            position: 0,
            expected_metadata: StreamMetadata {
                sample_rate,
                channels,
                duration: Duration::from_millis(duration_ms),
            },
        }
    }
}

impl Decoder for FakeDecoder {
    fn probe(&self) -> ProbeResult {
        ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<StreamMetadata, pulseseek_domain::decoder::DecodeError> {
        Ok(self.expected_metadata.clone())
    }

    fn read(&mut self, buf: &mut [f32]) -> Result<usize, pulseseek_domain::decoder::DecodeError> {
        let remaining = self.data.len() - self.position;
        let to_copy = buf.len().min(remaining);
        buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
        self.position += to_copy;
        Ok(to_copy)
    }

    fn seek(
        &mut self,
        target: SeekTarget,
    ) -> Result<Position, pulseseek_domain::decoder::DecodeError> {
        // Simulate seek by converting target position (ms) to sample offset.
        let samples_per_ms = (self.expected_metadata.sample_rate as usize
            * self.expected_metadata.channels as usize)
            / 1000;
        self.position = target.position().as_millis() as usize * samples_per_ms;
        if self.position > self.data.len() {
            self.position = self.data.len();
        }
        Ok(target.position())
    }
}

#[test]
fn fake_decoder_probe_returns_supported() {
    let decoder = FakeDecoder::new(vec![0.0; 100], 44100, 2, 1000);
    assert_eq!(decoder.probe(), ProbeResult::Supported);
}

#[test]
fn fake_decoder_metadata_round_trip() {
    let mut decoder = FakeDecoder::new(vec![0.0; 100], 48000, 1, 2000);
    let meta = decoder.metadata().expect("metadata should succeed");
    assert_eq!(meta.sample_rate, 48000);
    assert_eq!(meta.channels, 1);
    assert_eq!(meta.duration, Duration::from_millis(2000));
}

#[test]
fn fake_decoder_read_returns_frames() {
    let data: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
    let mut decoder = FakeDecoder::new(data, 44100, 1, 100);
    let mut buf = vec![0.0f32; 50];
    let frames = decoder.read(&mut buf).expect("read should succeed");
    assert_eq!(frames, 50);
    assert!((buf[0] - 0.0).abs() < 1e-6);
    assert!((buf[49] - 0.49).abs() < 1e-6);
}

#[test]
fn fake_decoder_read_returns_less_at_end() {
    let data: Vec<f32> = vec![1.0; 10];
    let mut decoder = FakeDecoder::new(data, 44100, 1, 10);
    let mut buf = vec![0.0f32; 50];
    let frames = decoder.read(&mut buf).expect("read should succeed");
    assert_eq!(frames, 10);
}

#[test]
fn fake_decoder_seek_then_read_returns_new_data() {
    let data: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
    let mut decoder = FakeDecoder::new(data, 1000, 1, 100); // 1 sample per ms
    let target =
        Duration::from_millis(50).seek_to(Position::from_millis(25)).expect("valid seek target");
    let pos = decoder.seek(target).expect("seek should succeed");
    assert_eq!(pos.as_millis(), 25);

    let mut buf = vec![0.0f32; 10];
    let frames = decoder.read(&mut buf).expect("read after seek");
    assert_eq!(frames, 10);
    assert!((buf[0] - 0.25).abs() < 1e-6);
}

#[test]
fn decode_error_implements_error_contract() {
    use pulseseek_domain::decoder::DecodeError;
    use pulseseek_domain::error::{DiagnosticCode, DiagnosticContext, ErrorContract};

    let source = std::io::Error::other("test");
    let err = DecodeError::new(DiagnosticContext::new(DiagnosticCode::BrowserRead), source);
    let desc = err.user_descriptor();
    assert!(!desc.message().is_empty(), "error should have a safe message");
}
