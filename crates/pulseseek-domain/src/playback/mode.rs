/// Playback mode determining end-of-file behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackMode {
    OneShot,
    LoopCurrent,
    Sequential,
    Random,
}

/// Action to take when a track reaches its end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NextAction {
    /// Stop playback entirely.
    Stop,
    /// Replay the current track from the beginning.
    Replay,
    /// Advance to the next track.
    Advance,
}

/// Determines the next action at end-of-file based on mode.
///
/// # Arguments
/// * `mode` - The current playback mode.
/// * `has_next` - Whether there is a next track (relevant for Sequential mode).
pub fn end_of_file_decision(mode: &PlaybackMode, has_next: bool) -> NextAction {
    match mode {
        PlaybackMode::OneShot => NextAction::Stop,
        PlaybackMode::LoopCurrent => NextAction::Replay,
        PlaybackMode::Sequential if has_next => NextAction::Advance,
        PlaybackMode::Sequential => NextAction::Stop,
        PlaybackMode::Random => NextAction::Advance,
    }
}
