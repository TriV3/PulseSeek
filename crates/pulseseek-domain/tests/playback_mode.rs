use pulseseek_domain::playback::mode::{end_of_file_decision, NextAction, PlaybackMode};

#[test]
fn one_shot_stops_at_end() {
    assert_eq!(end_of_file_decision(&PlaybackMode::OneShot, true), NextAction::Stop);
    assert_eq!(end_of_file_decision(&PlaybackMode::OneShot, false), NextAction::Stop);
}

#[test]
fn loop_current_replays_at_end() {
    assert_eq!(end_of_file_decision(&PlaybackMode::LoopCurrent, true), NextAction::Replay);
    assert_eq!(end_of_file_decision(&PlaybackMode::LoopCurrent, false), NextAction::Replay);
}

#[test]
fn sequential_advances_when_next_exists() {
    assert_eq!(end_of_file_decision(&PlaybackMode::Sequential, true), NextAction::Advance,);
}

#[test]
fn sequential_stops_when_no_next() {
    assert_eq!(end_of_file_decision(&PlaybackMode::Sequential, false), NextAction::Stop,);
}

#[test]
fn random_advances_at_end() {
    assert_eq!(end_of_file_decision(&PlaybackMode::Random, true), NextAction::Advance);
    assert_eq!(end_of_file_decision(&PlaybackMode::Random, false), NextAction::Advance);
}
