use pulseseek_domain::decoder::{DecodeError, Decoder, ProbeResult, StreamMetadata};
use pulseseek_domain::playback::position::{Position, SeekTarget};
use pulseseek_domain::visualization::VisualizationFrame;
use pulseseek_playback::{visualization_channel, PlaybackEngine, PublishResult, VisualizationTap};

fn frame(sequence: u64) -> VisualizationFrame {
    VisualizationFrame::new(sequence, sequence * 4, 48_000, 1, &[sequence as f32]).unwrap()
}

#[test]
fn channel_enforces_capacity_and_drops_incoming_frames() {
    let (mut publisher, mut subscriber) = visualization_channel(2);

    assert_eq!(publisher.try_publish(frame(1)), PublishResult::Published);
    assert_eq!(publisher.try_publish(frame(2)), PublishResult::Published);
    assert_eq!(publisher.try_publish(frame(3)), PublishResult::DroppedFull);
    assert_eq!(publisher.dropped_frames(), 1);
    assert_eq!(subscriber.try_receive().unwrap().sequence(), 1);
    assert_eq!(subscriber.try_receive().unwrap().sequence(), 2);
    assert!(subscriber.try_receive().is_none());
}

#[test]
fn removing_subscriber_closes_publisher_without_blocking() {
    let (mut publisher, subscriber) = visualization_channel(1);
    drop(subscriber);

    assert_eq!(publisher.try_publish(frame(1)), PublishResult::SubscriberGone);
}

#[test]
fn shutdown_is_visible_to_both_channel_ends() {
    let (mut publisher, subscriber) = visualization_channel(1);

    publisher.shutdown();

    assert!(publisher.is_shutdown());
    assert!(subscriber.is_shutdown());
    assert_eq!(publisher.try_publish(frame(1)), PublishResult::Shutdown);
}

#[test]
fn dropping_publisher_closes_subscriber_after_buffer_is_drained() {
    let (mut publisher, mut subscriber) = visualization_channel(1);
    assert_eq!(publisher.try_publish(frame(1)), PublishResult::Published);
    drop(publisher);

    assert_eq!(subscriber.try_receive().unwrap().sequence(), 1);
    assert!(subscriber.try_receive().is_none());
    assert!(subscriber.is_closed());
}

#[test]
fn saturated_visualization_tap_does_not_change_playback_output() {
    let (publisher, mut subscriber) = visualization_channel(1);
    let tap = VisualizationTap::new(publisher, 48_000, 1, 2).unwrap();
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(FixedDecoder::new()), 8);
    consumer.set_visualization_tap(tap);
    assert!(engine.process_chunk().unwrap());

    let mut output = [0.0; 4];
    let written = consumer.consume_channels_with_volume(&mut output, 1, 1, 1.0);

    assert_eq!(written, 4);
    assert_eq!(output, [0.25, 0.5, 0.75, 1.0]);
    let published = subscriber.try_receive().unwrap();
    assert_eq!(published.samples(), &[0.25, 0.5]);
    assert!(subscriber.try_receive().is_none());
    assert_eq!(consumer.visualization_dropped_frames(), Some(1));
}

#[test]
fn visualization_tap_ignores_output_with_a_different_channel_layout() {
    let (publisher, mut subscriber) = visualization_channel(1);
    let tap = VisualizationTap::new(publisher, 48_000, 1, 2).unwrap();
    let (mut engine, mut consumer) = PlaybackEngine::new(Box::new(FixedDecoder::new()), 8);
    consumer.set_visualization_tap(tap);
    assert!(engine.process_chunk().unwrap());

    let mut output = [0.0; 4];
    let written = consumer.consume_channels_with_volume(&mut output, 1, 2, 1.0);

    assert_eq!(written, 4);
    assert_eq!(output, [0.25, 0.25, 0.5, 0.5]);
    assert!(subscriber.try_receive().is_none());
}

struct FixedDecoder {
    samples: [f32; 4],
    emitted: bool,
}

impl FixedDecoder {
    fn new() -> Self {
        Self { samples: [0.25, 0.5, 0.75, 1.0], emitted: false }
    }
}

impl Decoder for FixedDecoder {
    fn probe(&self) -> ProbeResult {
        ProbeResult::Supported
    }

    fn metadata(&mut self) -> Result<StreamMetadata, DecodeError> {
        unimplemented!("not used by playback contract test")
    }

    fn read(&mut self, buffer: &mut [f32]) -> Result<usize, DecodeError> {
        if self.emitted {
            return Ok(0);
        }
        buffer[..self.samples.len()].copy_from_slice(&self.samples);
        self.emitted = true;
        Ok(self.samples.len())
    }

    fn seek(&mut self, _target: SeekTarget) -> Result<Position, DecodeError> {
        unimplemented!("not used by playback contract test")
    }
}
