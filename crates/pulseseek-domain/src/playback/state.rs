use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Loading,
    Playing,
    Paused,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackCommand {
    Load,
    Play,
    Pause,
    Resume,
    Stop,
    Fail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    InvalidTransition { from: PlaybackState, command: PlaybackCommand },
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitionError::InvalidTransition { from, command } => {
                write!(f, "cannot {command:?} from {from:?}")
            },
        }
    }
}

pub fn transition(
    state: &PlaybackState,
    command: &PlaybackCommand,
) -> Result<PlaybackState, TransitionError> {
    match (state, command) {
        (PlaybackState::Stopped, PlaybackCommand::Load) => Ok(PlaybackState::Loading),
        (PlaybackState::Stopped, PlaybackCommand::Fail) => Ok(PlaybackState::Failed),

        (PlaybackState::Loading, PlaybackCommand::Play) => Ok(PlaybackState::Playing),
        (PlaybackState::Loading, PlaybackCommand::Stop) => Ok(PlaybackState::Stopped),
        (PlaybackState::Loading, PlaybackCommand::Fail) => Ok(PlaybackState::Failed),

        (PlaybackState::Playing, PlaybackCommand::Pause) => Ok(PlaybackState::Paused),
        (PlaybackState::Playing, PlaybackCommand::Stop) => Ok(PlaybackState::Stopped),
        (PlaybackState::Playing, PlaybackCommand::Fail) => Ok(PlaybackState::Failed),

        (PlaybackState::Paused, PlaybackCommand::Resume) => Ok(PlaybackState::Playing),
        (PlaybackState::Paused, PlaybackCommand::Stop) => Ok(PlaybackState::Stopped),
        (PlaybackState::Paused, PlaybackCommand::Fail) => Ok(PlaybackState::Failed),

        (PlaybackState::Failed, PlaybackCommand::Load) => Ok(PlaybackState::Loading),
        (PlaybackState::Failed, PlaybackCommand::Stop) => Ok(PlaybackState::Stopped),

        (from, command) => {
            Err(TransitionError::InvalidTransition { from: *from, command: *command })
        },
    }
}
