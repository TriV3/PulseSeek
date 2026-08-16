use pulseseek_domain::playback::loop_region::{LoopRegion, LoopRegionError, LoopRegionState};
use pulseseek_domain::playback::position::{Duration, Position};

fn ms(value: u64) -> Position {
    Position::from_millis(value)
}

// Ordering
#[test]
fn valid_region_round_trips() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    assert_eq!(region.start(), ms(100));
    assert_eq!(region.end(), ms(500));
}

#[test]
fn reversed_points_rejected() {
    let err = LoopRegion::new(ms(500), ms(100), Duration::from_millis(1000))
        .expect_err("reversed points should fail");
    assert_eq!(err, LoopRegionError::ZeroLength { start: ms(500), end: ms(100) });
}

// Equal points
#[test]
fn equal_points_rejected() {
    let err = LoopRegion::new(ms(100), ms(100), Duration::from_millis(1000))
        .expect_err("equal points should fail");
    assert_eq!(err, LoopRegionError::ZeroLength { start: ms(100), end: ms(100) });
}

// Bounds
#[test]
fn region_ending_at_duration_accepted() {
    let region =
        LoopRegion::new(ms(0), ms(1000), Duration::from_millis(1000)).expect("end at duration");
    assert_eq!(region.end(), ms(1000));
}

#[test]
fn end_beyond_duration_rejected() {
    let err = LoopRegion::new(ms(100), ms(1001), Duration::from_millis(1000))
        .expect_err("end beyond duration should fail");
    assert_eq!(err, LoopRegionError::OutOfBounds { position: ms(1001), max: ms(1000) });
}

#[test]
fn start_beyond_duration_rejected() {
    let err = LoopRegion::new(ms(1001), ms(2000), Duration::from_millis(1000))
        .expect_err("start beyond duration should fail");
    assert_eq!(err, LoopRegionError::OutOfBounds { position: ms(1001), max: ms(1000) });
}

#[test]
fn unknown_duration_rejected() {
    let err = LoopRegion::new(ms(0), ms(100), Duration::Unknown)
        .expect_err("unknown duration should fail");
    assert_eq!(err, LoopRegionError::UnknownDuration);
}

// Region membership
#[test]
fn contains_respects_half_open_bounds() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    assert!(!region.contains(ms(99)), "before start");
    assert!(region.contains(ms(100)), "start is inclusive");
    assert!(region.contains(ms(250)), "inside");
    assert!(region.contains(ms(499)), "just before end");
    assert!(!region.contains(ms(500)), "end is exclusive");
}

// Clear
#[test]
fn clear_removes_region() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    let state = LoopRegionState::Set(region);
    assert_eq!(state.clear(), LoopRegionState::None);
}

// Duration change
#[test]
fn revalidate_shrunk_duration_drops_region() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    let state = LoopRegionState::Set(region).revalidate(Duration::from_millis(300));
    assert_eq!(state, LoopRegionState::None);
}

#[test]
fn revalidate_grown_duration_keeps_region() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    let state = LoopRegionState::Set(region).revalidate(Duration::from_millis(2000));
    assert_eq!(state, LoopRegionState::Set(region));
}

#[test]
fn revalidate_unknown_duration_drops_region() {
    let region =
        LoopRegion::new(ms(100), ms(500), Duration::from_millis(1000)).expect("valid region");
    let state = LoopRegionState::Set(region).revalidate(Duration::Unknown);
    assert_eq!(state, LoopRegionState::None);
}

#[test]
fn revalidate_none_stays_none() {
    let state = LoopRegionState::None.revalidate(Duration::from_millis(1000));
    assert_eq!(state, LoopRegionState::None);
}

// Errors
#[test]
fn zero_length_error_display_mentions_both_points() {
    let err = LoopRegionError::ZeroLength { start: ms(100), end: ms(100) };
    let msg = err.to_string();
    assert!(msg.contains("100"), "display should mention start");
    assert!(msg.contains("before"), "display should describe ordering");
}

#[test]
fn out_of_bounds_error_display_mentions_position_and_max() {
    let err = LoopRegionError::OutOfBounds { position: ms(1001), max: ms(1000) };
    let msg = err.to_string();
    assert!(msg.contains("1001"), "display should mention position");
    assert!(msg.contains("1000"), "display should mention max");
}

#[test]
fn loop_region_error_implements_std_error() {
    use std::error::Error;
    let err = LoopRegionError::ZeroLength { start: ms(100), end: ms(100) };
    // Must compile — verifies Error trait bound.
    let _: &dyn Error = &err;
}
