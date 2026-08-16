use pulseseek_domain::playback::volume::{Gain, Mute, Volume};

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// Gain boundary
#[test]
fn gain_at_zero() {
    assert!(approx_eq(Gain::new(0.0).as_f64(), 0.0));
}

#[test]
fn gain_at_unity() {
    assert!(approx_eq(Gain::new(1.0).as_f64(), 1.0));
}

#[test]
fn gain_clamps_negative() {
    assert!(approx_eq(Gain::new(-1.0).as_f64(), 0.0));
}

#[test]
fn gain_clamps_above_max() {
    assert!(approx_eq(Gain::new(3.0).as_f64(), Gain::MAX));
}

#[test]
fn gain_handles_nan() {
    assert!(approx_eq(Gain::new(f64::NAN).as_f64(), 0.0));
}

#[test]
fn gain_round_trip() {
    let g = Gain::new(0.5);
    assert!(approx_eq(g.as_f64(), 0.5));
    assert_eq!(g, Gain::new(0.5));
}

// Mute / unmute
#[test]
fn volume_default_not_muted() {
    let v = Volume::new(Gain::new(0.5));
    assert_eq!(v.mute(), Mute::Off);
}

#[test]
fn volume_muted_state() {
    let v = Volume::muted();
    assert_eq!(v.mute(), Mute::On);
}

#[test]
fn volume_mute_round_trip() {
    let v = Volume::muted().with_mute(Mute::Off);
    assert_eq!(v.mute(), Mute::Off);
}

// Effective gain
#[test]
fn effective_gain_zero_when_muted() {
    let v = Volume::muted();
    assert!(approx_eq(v.effective_gain(), 0.0));
}

#[test]
fn effective_gain_matches_gain_when_unmuted() {
    let v = Volume::new(Gain::new(0.5));
    assert!(approx_eq(v.effective_gain(), 0.5));
}
