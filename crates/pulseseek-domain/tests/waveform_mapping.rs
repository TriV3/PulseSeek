use pulseseek_domain::playback::position::{Duration, Position};
use pulseseek_domain::waveform::mapping::{Timeline, TimelineError};

fn timeline(width_px: u64, duration_ms: u64) -> Timeline {
    Timeline::new(width_px, Duration::from_millis(duration_ms)).expect("valid timeline")
}

fn pos(ms: u64) -> Position {
    Position::from_millis(ms)
}

// ── Construction ───────────────────────────────────────────────────

#[test]
fn timeline_zero_width_rejected() {
    let err = Timeline::new(0, Duration::from_millis(1000)).expect_err("zero width rejected");
    assert_eq!(err, TimelineError::ZeroWidth);
}

#[test]
fn timeline_unknown_duration_rejected() {
    let err = Timeline::new(100, Duration::Unknown).expect_err("unknown duration rejected");
    assert_eq!(err, TimelineError::UnknownDuration);
}

#[test]
fn timeline_accessors() {
    let t = timeline(100, 5_000);
    assert_eq!(t.width_px(), 100);
    assert_eq!(t.duration(), pos(5_000));
}

// ── Boundaries ─────────────────────────────────────────────────────

#[test]
fn pixel_zero_maps_to_time_zero() {
    let t = timeline(100, 5_000);
    assert_eq!(t.position_at(0), pos(0));
}

#[test]
fn last_pixel_maps_to_full_duration() {
    let t = timeline(100, 5_000);
    assert_eq!(t.position_at(99), pos(5_000));
}

#[test]
fn time_zero_maps_to_first_pixel() {
    let t = timeline(100, 5_000);
    assert_eq!(t.pixel_for(pos(0)), 0);
}

#[test]
fn full_duration_maps_to_last_pixel() {
    let t = timeline(100, 5_000);
    assert_eq!(t.pixel_for(pos(5_000)), 99);
}

// ── Clamping ───────────────────────────────────────────────────────

#[test]
fn negative_pixel_clamps_to_start() {
    let t = timeline(100, 5_000);
    assert_eq!(t.position_at(-100), pos(0));
    assert_eq!(t.position_at(i64::MIN), pos(0));
}

#[test]
fn pixel_beyond_width_clamps_to_duration() {
    let t = timeline(100, 5_000);
    assert_eq!(t.position_at(100), pos(5_000));
    assert_eq!(t.position_at(i64::MAX), pos(5_000));
}

#[test]
fn position_beyond_duration_clamps_to_last_pixel() {
    let t = timeline(100, 5_000);
    assert_eq!(t.pixel_for(pos(5_500)), 99);
    assert_eq!(t.pixel_for(pos(u64::MAX)), 99);
}

// ── Degenerate geometries ──────────────────────────────────────────

#[test]
fn width_one_maps_everything_to_zero() {
    let t = timeline(1, 5_000);
    assert_eq!(t.position_at(0), pos(0));
    assert_eq!(t.position_at(99), pos(0));
    assert_eq!(t.pixel_for(pos(0)), 0);
    assert_eq!(t.pixel_for(pos(5_000)), 0);
    assert_eq!(t.pixel_for(pos(2_500)), 0);
}

#[test]
fn zero_duration_maps_everything_to_zero() {
    let t = timeline(100, 0);
    assert_eq!(t.position_at(0), pos(0));
    assert_eq!(t.position_at(99), pos(0));
    assert_eq!(t.pixel_for(pos(0)), 0);
    assert_eq!(t.pixel_for(pos(50)), 0);
}

// ── Overflow safety ────────────────────────────────────────────────

#[test]
fn huge_values_do_not_overflow() {
    let t = timeline(u64::MAX, u64::MAX);
    assert_eq!(t.pixel_for(pos(u64::MAX)), u64::MAX - 1);

    let width = 1u64 << 40;
    let t2 = timeline(width, u64::MAX);
    assert_eq!(t2.position_at((width - 1) as i64), pos(u64::MAX));
}

// ── Midpoint and rounding ──────────────────────────────────────────

#[test]
fn midpoint_maps_to_midpoint() {
    let t = timeline(1001, 1_000);
    assert_eq!(t.pixel_for(pos(500)), 500);
    assert_eq!(t.position_at(500), pos(500));
}

// ── Property tests ─────────────────────────────────────────────────

#[test]
fn pixel_mapping_is_monotonic_over_positions() {
    let t = timeline(800, 60_000);
    let mut prev = 0;
    for ms in 0..=60_000u64 {
        let px = t.pixel_for(pos(ms));
        assert!(px >= prev, "pixel_for decreased at {ms}ms");
        prev = px;
    }
}

#[test]
fn time_mapping_is_monotonic_over_pixels() {
    let t = timeline(800, 60_000);
    let mut prev = pos(0);
    for x in 0..=799i64 {
        let p = t.position_at(x);
        assert!(p >= prev, "position_at decreased at pixel {x}");
        prev = p;
    }
}

#[test]
fn position_pixel_round_trip_stays_within_one_bucket() {
    let width = 800u64;
    let duration_ms = 60_000u64;
    let t = timeline(width, duration_ms);
    let bucket = duration_ms / (width - 1) + 1;
    let mut ms = 0u64;
    while ms <= duration_ms {
        let px = t.pixel_for(pos(ms));
        let back = t.position_at(px as i64);
        let diff = (back.as_millis() as i128 - ms as i128).unsigned_abs();
        assert!(diff <= bucket as u128, "round trip drifted {diff}ms at {ms}ms");
        ms += 37;
    }
}

#[test]
fn pixel_position_round_trip_stays_within_one_bucket() {
    let width = 800u64;
    let duration_ms = 60_000u64;
    let t = timeline(width, duration_ms);
    for x in 0..=799i64 {
        let p = t.position_at(x);
        let px = t.pixel_for(p);
        assert!((px as i64 - x).unsigned_abs() <= 1, "pixel drifted at pixel {x}");
    }
}
