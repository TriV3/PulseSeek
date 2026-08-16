use pulseseek_domain::playback::state::{
    transition, PlaybackCommand, PlaybackState, TransitionError,
};

#[test]
fn stopped_can_transition_to_loading() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Load),
        Ok(PlaybackState::Loading),
    );
}

#[test]
fn stopped_can_transition_to_failed() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Fail),
        Ok(PlaybackState::Failed),
    );
}

#[test]
fn loading_can_transition_to_playing() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Play),
        Ok(PlaybackState::Playing),
    );
}

#[test]
fn loading_can_transition_to_stopped() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Stop),
        Ok(PlaybackState::Stopped),
    );
}

#[test]
fn loading_can_transition_to_failed() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Fail),
        Ok(PlaybackState::Failed),
    );
}

#[test]
fn playing_can_transition_to_paused() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Pause),
        Ok(PlaybackState::Paused),
    );
}

#[test]
fn playing_can_transition_to_stopped() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Stop),
        Ok(PlaybackState::Stopped),
    );
}

#[test]
fn playing_can_transition_to_failed() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Fail),
        Ok(PlaybackState::Failed),
    );
}

#[test]
fn paused_can_transition_to_playing() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Resume),
        Ok(PlaybackState::Playing),
    );
}

#[test]
fn paused_can_transition_to_stopped() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Stop),
        Ok(PlaybackState::Stopped),
    );
}

#[test]
fn paused_can_transition_to_failed() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Fail),
        Ok(PlaybackState::Failed),
    );
}

#[test]
fn failed_can_transition_to_loading() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Load),
        Ok(PlaybackState::Loading),
    );
}

#[test]
fn failed_can_transition_to_stopped() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Stop),
        Ok(PlaybackState::Stopped),
    );
}

#[test]
fn stopped_rejects_play() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Play),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Stopped,
            command: PlaybackCommand::Play,
        }),
    );
}

#[test]
fn stopped_rejects_pause() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Pause),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Stopped,
            command: PlaybackCommand::Pause,
        }),
    );
}

#[test]
fn stopped_rejects_resume() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Resume),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Stopped,
            command: PlaybackCommand::Resume,
        }),
    );
}

#[test]
fn stopped_rejects_stop() {
    assert_eq!(
        transition(&PlaybackState::Stopped, &PlaybackCommand::Stop),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Stopped,
            command: PlaybackCommand::Stop,
        }),
    );
}

#[test]
fn loading_rejects_pause() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Pause),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Loading,
            command: PlaybackCommand::Pause,
        }),
    );
}

#[test]
fn loading_rejects_resume() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Resume),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Loading,
            command: PlaybackCommand::Resume,
        }),
    );
}

#[test]
fn loading_rejects_load() {
    assert_eq!(
        transition(&PlaybackState::Loading, &PlaybackCommand::Load),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Loading,
            command: PlaybackCommand::Load,
        }),
    );
}

#[test]
fn playing_rejects_play() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Play),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Playing,
            command: PlaybackCommand::Play,
        }),
    );
}

#[test]
fn playing_rejects_resume() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Resume),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Playing,
            command: PlaybackCommand::Resume,
        }),
    );
}

#[test]
fn playing_rejects_load() {
    assert_eq!(
        transition(&PlaybackState::Playing, &PlaybackCommand::Load),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Playing,
            command: PlaybackCommand::Load,
        }),
    );
}

#[test]
fn paused_rejects_pause() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Pause),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Paused,
            command: PlaybackCommand::Pause,
        }),
    );
}

#[test]
fn paused_rejects_play() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Play),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Paused,
            command: PlaybackCommand::Play,
        }),
    );
}

#[test]
fn paused_rejects_load() {
    assert_eq!(
        transition(&PlaybackState::Paused, &PlaybackCommand::Load),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Paused,
            command: PlaybackCommand::Load,
        }),
    );
}

#[test]
fn failed_rejects_play() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Play),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Failed,
            command: PlaybackCommand::Play,
        }),
    );
}

#[test]
fn failed_rejects_pause() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Pause),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Failed,
            command: PlaybackCommand::Pause,
        }),
    );
}

#[test]
fn failed_rejects_resume() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Resume),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Failed,
            command: PlaybackCommand::Resume,
        }),
    );
}

#[test]
fn failed_rejects_fail() {
    assert_eq!(
        transition(&PlaybackState::Failed, &PlaybackCommand::Fail),
        Err(TransitionError::InvalidTransition {
            from: PlaybackState::Failed,
            command: PlaybackCommand::Fail,
        }),
    );
}

#[test]
fn transition_is_deterministic() {
    let states = [
        PlaybackState::Stopped,
        PlaybackState::Loading,
        PlaybackState::Playing,
        PlaybackState::Paused,
        PlaybackState::Failed,
    ];
    let commands = [
        PlaybackCommand::Load,
        PlaybackCommand::Play,
        PlaybackCommand::Pause,
        PlaybackCommand::Resume,
        PlaybackCommand::Stop,
        PlaybackCommand::Fail,
    ];

    for state in &states {
        for command in &commands {
            let first = transition(state, command);
            let second = transition(state, command);
            assert_eq!(first, second, "non-deterministic transition for {state:?} + {command:?}");
        }
    }
}
