/// Terminal playback outcome for one-shot playback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackEvent {
    Completed,
    Failed,
}
