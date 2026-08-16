use pulseseek_domain::waveform::peak::Peak;

#[test]
fn peak_zero_amplitude() {
    let p = Peak::from_parts(0.0, 0.0);
    assert_eq!(p.min(), 0.0);
    assert_eq!(p.max(), 0.0);
}

#[test]
fn peak_typical_amplitude() {
    let p = Peak::from_parts(-0.5, 0.5);
    assert_eq!(p.min(), -0.5);
    assert_eq!(p.max(), 0.5);
}

#[test]
fn peak_clamps_below_amplitude_min() {
    let p = Peak::from_parts(-2.0, -0.5);
    assert_eq!(p.min(), Peak::AMPLITUDE_MIN);
    assert_eq!(p.max(), -0.5);
}

#[test]
fn peak_clamps_above_amplitude_max() {
    let p = Peak::from_parts(0.5, 2.0);
    assert_eq!(p.min(), 0.5);
    assert_eq!(p.max(), Peak::AMPLITUDE_MAX);
}

#[test]
fn peak_orders_reversed_parts() {
    let p = Peak::from_parts(0.8, -0.3);
    assert_eq!(p.min(), -0.3);
    assert_eq!(p.max(), 0.8);
}

#[test]
fn peak_nan_clamps_to_zero() {
    let p = Peak::from_parts(f32::NAN, 0.5);
    assert_eq!(p.min(), 0.0);
    assert_eq!(p.max(), 0.5);

    let q = Peak::from_parts(-0.5, f32::NAN);
    assert_eq!(q.min(), -0.5);
    assert_eq!(q.max(), 0.0);
}

#[test]
fn peak_round_trip_identity() {
    let p = Peak::from_parts(-0.25, 0.75);
    let again = Peak::from_parts(p.min(), p.max());
    assert_eq!(p, again);
}

#[test]
fn peak_equality() {
    assert_eq!(Peak::from_parts(-1.0, 1.0), Peak::from_parts(-1.0, 1.0));
    assert_ne!(Peak::from_parts(-1.0, 1.0), Peak::from_parts(-0.5, 1.0));
}

// Property: every constructed peak is bounded and ordered regardless of input.
#[test]
fn peak_always_bounded_and_ordered() {
    for i in 0..=40u32 {
        let v = -2.0 + i as f32 * 0.1;
        let p = Peak::from_parts(-v, v);
        assert!(p.min() >= Peak::AMPLITUDE_MIN, "min below bound at {v}");
        assert!(p.max() <= Peak::AMPLITUDE_MAX, "max above bound at {v}");
        assert!(p.min() <= p.max(), "min above max at {v}");
    }
}
