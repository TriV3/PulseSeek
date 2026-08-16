use pulseseek_domain::playback::position::{Duration, Position, SeekError};

#[test]
fn position_from_millis_round_trips() {
    let pos = Position::from_millis(5000);
    assert_eq!(pos.as_millis(), 5000);
}

#[test]
fn position_ordering() {
    let a = Position::from_millis(1);
    let b = Position::from_millis(2);
    assert!(a < b);
}

#[test]
fn position_zero_is_valid() {
    let pos = Position::from_millis(0);
    assert_eq!(pos.as_millis(), 0);
}

#[test]
fn duration_unknown_accepts_any_seek() {
    let dur = Duration::Unknown;
    let target =
        dur.seek_to(Position::from_millis(u64::MAX)).expect("unknown duration accepts any");
    assert_eq!(target.position(), Position::from_millis(u64::MAX));
}

#[test]
fn duration_finite_seek_in_range() {
    let dur = Duration::from_millis(1000);
    let target = dur.seek_to(Position::from_millis(500)).expect("seek within range");
    assert_eq!(target.position(), Position::from_millis(500));
}

#[test]
fn duration_finite_seek_at_start() {
    let dur = Duration::from_millis(1000);
    let target = dur.seek_to(Position::from_millis(0)).expect("seek at start");
    assert_eq!(target.position(), Position::from_millis(0));
}

#[test]
fn duration_finite_seek_at_end() {
    let dur = Duration::from_millis(1000);
    let target = dur.seek_to(Position::from_millis(1000)).expect("seek at end");
    assert_eq!(target.position(), Position::from_millis(1000));
}

#[test]
fn duration_finite_seek_beyond_rejected() {
    let dur = Duration::from_millis(1000);
    let err = dur.seek_to(Position::from_millis(1001)).expect_err("seek beyond should fail");
    assert_eq!(err.requested, Position::from_millis(1001));
    assert_eq!(err.max, Position::from_millis(1000));
}

#[test]
fn duration_finite_seek_overflow_rejected() {
    let dur = Duration::from_millis(100);
    let err = dur.seek_to(Position::from_millis(u64::MAX)).expect_err("overflow seek should fail");
    assert_eq!(err.requested, Position::from_millis(u64::MAX));
    assert_eq!(err.max, Position::from_millis(100));
}

#[test]
fn duration_zero_rejects_positive_seek() {
    let dur = Duration::from_millis(0);
    let err =
        dur.seek_to(Position::from_millis(1)).expect_err("seek beyond zero duration should fail");
    assert_eq!(err.requested, Position::from_millis(1));
    assert_eq!(err.max, Position::from_millis(0));
}

#[test]
fn seek_error_display_mentions_requested_and_max() {
    let err =
        SeekError { requested: Position::from_millis(5000), max: Position::from_millis(3000) };
    let msg = err.to_string();
    assert!(msg.contains("5000"), "display should mention requested");
    assert!(msg.contains("3000"), "display should mention max");
}

#[test]
fn seek_error_implements_std_error() {
    use std::error::Error;
    let err =
        SeekError { requested: Position::from_millis(5000), max: Position::from_millis(3000) };
    // Must compile — verifies Error trait bound.
    let _: &dyn Error = &err;
}

#[test]
fn seek_target_ordering() {
    let dur = Duration::from_millis(100);
    let early = dur.seek_to(Position::from_millis(10)).unwrap();
    let late = dur.seek_to(Position::from_millis(50)).unwrap();
    assert!(early < late);
}
