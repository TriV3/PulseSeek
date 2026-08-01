use pulseseek_domain::waveform::levels::{
    Level, LevelIndex, LevelIndexError, MultiresolutionWaveform, WaveformError, MAX_LEVELS,
};
use pulseseek_domain::waveform::peak::Peak;

fn peak(min: f32, max: f32) -> Peak {
    Peak::from_parts(min, max)
}

fn level(index: u32, samples_per_peak: u64, peak_count: usize) -> Level {
    Level {
        index: LevelIndex::new(index).expect("valid index"),
        samples_per_peak,
        peaks: (0..peak_count).map(|_| peak(-1.0, 1.0)).collect(),
    }
}

fn waveform(channels: u16, levels: Vec<Level>) -> MultiresolutionWaveform {
    MultiresolutionWaveform::from_levels(channels, levels).expect("valid waveform")
}

// ── LevelIndex ─────────────────────────────────────────────────────

#[test]
fn level_index_zero_is_valid() {
    let idx = LevelIndex::new(0).expect("zero is valid");
    assert_eq!(idx.value(), 0);
}

#[test]
fn level_index_max_is_valid() {
    let idx = LevelIndex::new(MAX_LEVELS - 1).expect("max is valid");
    assert_eq!(idx.value(), MAX_LEVELS - 1);
}

#[test]
fn level_index_at_max_rejected() {
    let err = LevelIndex::new(MAX_LEVELS).expect_err("max bound rejected");
    assert_eq!(err.value, MAX_LEVELS);
}

#[test]
fn level_index_beyond_max_rejected() {
    let err = LevelIndex::new(1_000).expect_err("out of range rejected");
    assert_eq!(err.value, 1_000);
}

#[test]
fn level_index_ordering() {
    let low = LevelIndex::new(0).expect("valid");
    let high = LevelIndex::new(1).expect("valid");
    assert!(low < high);
}

#[test]
fn level_index_error_display_mentions_value() {
    let err = LevelIndexError { value: 8 };
    assert!(err.to_string().contains("8"));
}

#[test]
fn level_index_error_implements_std_error() {
    use std::error::Error;
    let err = LevelIndexError { value: 8 };
    let _: &dyn Error = &err;
}

// ── MultiresolutionWaveform construction ───────────────────────────

#[test]
fn waveform_empty_rejected() {
    let err = MultiresolutionWaveform::from_levels(1, vec![]).expect_err("empty rejected");
    assert_eq!(err, WaveformError::Empty);
}

#[test]
fn waveform_zero_channels_rejected() {
    let levels = vec![level(0, 16, 4)];
    let err = MultiresolutionWaveform::from_levels(0, levels).expect_err("zero channels rejected");
    assert_eq!(err, WaveformError::ZeroChannels);
}

#[test]
fn waveform_non_contiguous_indices_rejected() {
    let levels = vec![level(0, 16, 4), level(2, 8, 8)];
    let err = MultiresolutionWaveform::from_levels(1, levels).expect_err("gap rejected");
    assert_eq!(err, WaveformError::NonContiguousIndices);
}

#[test]
fn waveform_starting_above_zero_rejected() {
    let levels = vec![level(1, 8, 8)];
    let err =
        MultiresolutionWaveform::from_levels(1, levels).expect_err("not starting at zero rejected");
    assert_eq!(err, WaveformError::NonContiguousIndices);
}

#[test]
fn waveform_level_with_zero_peaks_rejected() {
    let levels = vec![level(0, 16, 0)];
    let err = MultiresolutionWaveform::from_levels(1, levels).expect_err("zero peaks rejected");
    assert_eq!(err, WaveformError::ZeroPeaks);
}

#[test]
fn waveform_stereo_unaligned_peaks_rejected() {
    let levels = vec![level(0, 16, 3)];
    let err =
        MultiresolutionWaveform::from_levels(2, levels).expect_err("unaligned peaks rejected");
    assert_eq!(err, WaveformError::PeaksNotAlignedToChannels);
}

#[test]
fn waveform_level_with_zero_samples_per_peak_rejected() {
    let mut lvl = level(0, 16, 4);
    lvl.samples_per_peak = 0;
    let err =
        MultiresolutionWaveform::from_levels(1, vec![lvl]).expect_err("zero resolution rejected");
    assert_eq!(err, WaveformError::NonPositiveSamplesPerPeak);
}

#[test]
fn waveform_equal_resolution_rejected() {
    let levels = vec![level(0, 8, 4), level(1, 8, 8)];
    let err =
        MultiresolutionWaveform::from_levels(1, levels).expect_err("equal resolution rejected");
    assert_eq!(err, WaveformError::NonDecreasingResolution);
}

#[test]
fn waveform_finer_level_with_more_samples_per_peak_rejected() {
    let levels = vec![level(0, 4, 4), level(1, 8, 8)];
    let err = MultiresolutionWaveform::from_levels(1, levels)
        .expect_err("increasing resolution rejected");
    assert_eq!(err, WaveformError::NonDecreasingResolution);
}

#[test]
fn waveform_valid_pyramid_accepted() {
    let w = waveform(1, vec![level(0, 16, 4), level(1, 8, 8), level(2, 4, 16)]);
    assert_eq!(w.len(), 3);
    assert!(!w.is_empty());
}

#[test]
fn waveform_stereo_aligned_pyramid_accepted() {
    // Two channels, each level holds an even number of interleaved peaks.
    let w = waveform(2, vec![level(0, 16, 4), level(1, 8, 8), level(2, 4, 16)]);
    assert_eq!(w.channels(), 2);
    assert_eq!(w.len(), 3);
}

#[test]
fn waveform_single_level_accepted() {
    let w = waveform(1, vec![level(0, 4, 16)]);
    assert_eq!(w.len(), 1);
}

#[test]
fn waveform_channels_accessor() {
    let w = waveform(2, vec![level(0, 4, 8)]);
    assert_eq!(w.channels(), 2);
}

// ── Accessors ──────────────────────────────────────────────────────

#[test]
fn waveform_levels_ordered_coarsest_to_finest() {
    let w = waveform(1, vec![level(0, 16, 4), level(1, 8, 8), level(2, 4, 16)]);
    let indices: Vec<u32> = w.levels().iter().map(|l| l.index.value()).collect();
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn waveform_coarsest_is_first() {
    let w = waveform(1, vec![level(0, 16, 4), level(1, 8, 8)]);
    assert_eq!(w.coarsest().index.value(), 0);
    assert_eq!(w.coarsest().samples_per_peak, 16);
}

#[test]
fn waveform_finest_is_last() {
    let w = waveform(1, vec![level(0, 16, 4), level(1, 8, 8)]);
    assert_eq!(w.finest().index.value(), 1);
    assert_eq!(w.finest().samples_per_peak, 8);
}

#[test]
fn waveform_level_by_index() {
    let w = waveform(1, vec![level(0, 16, 4), level(1, 8, 8)]);
    assert_eq!(w.level(LevelIndex::new(0).expect("valid")).expect("present").samples_per_peak, 16);
    assert_eq!(w.level(LevelIndex::new(1).expect("valid")).expect("present").samples_per_peak, 8);
    assert!(w.level(LevelIndex::new(2).expect("valid")).is_none());
}

// ── Level selection ────────────────────────────────────────────────

fn four_eight_sixteen() -> MultiresolutionWaveform {
    waveform(1, vec![level(0, 16, 4), level(1, 8, 8), level(2, 4, 16)])
}

#[test]
fn select_level_target_one_returns_coarsest() {
    assert_eq!(four_eight_sixteen().select_level(1).index.value(), 0);
}

#[test]
fn select_level_target_zero_returns_coarsest() {
    assert_eq!(four_eight_sixteen().select_level(0).index.value(), 0);
}

#[test]
fn select_level_target_within_coarsest_returns_coarsest() {
    assert_eq!(four_eight_sixteen().select_level(4).index.value(), 0);
}

#[test]
fn select_level_target_beyond_coarsest_promotes() {
    assert_eq!(four_eight_sixteen().select_level(5).index.value(), 1);
    assert_eq!(four_eight_sixteen().select_level(8).index.value(), 1);
}

#[test]
fn select_level_target_beyond_middle_promotes() {
    assert_eq!(four_eight_sixteen().select_level(9).index.value(), 2);
    assert_eq!(four_eight_sixteen().select_level(16).index.value(), 2);
}

#[test]
fn select_level_target_beyond_finest_returns_finest() {
    assert_eq!(four_eight_sixteen().select_level(100).index.value(), 2);
}

// Property: selection never returns a level with fewer peaks than the
// target unless the finest level is the only option, and the selected
// index never decreases as the target grows.
#[test]
fn select_level_properties() {
    let w = four_eight_sixteen();
    let mut prev_index = 0;
    for target in 0..=100u64 {
        let selected = w.select_level(target);
        let count = selected.peaks.len() as u64;
        let is_finest = selected.index == w.finest().index;
        assert!(count >= target || is_finest, "target {target} under-served");
        assert!(selected.index.value() >= prev_index, "selection regressed at target {target}");
        prev_index = selected.index.value();
    }
}

// ── Stereo level selection ─────────────────────────────────────────

fn stereo_two_four_eight() -> MultiresolutionWaveform {
    // Interleaved peaks: 1 bucket = 2 peaks. Levels hold 2, 4, 8 buckets.
    waveform(2, vec![level(0, 16, 4), level(1, 8, 8), level(2, 4, 16)])
}

#[test]
fn select_level_counts_buckets_for_stereo() {
    let w = stereo_two_four_eight();
    // Coarsest has 2 buckets: serves any target up to 2.
    assert_eq!(w.select_level(1).index.value(), 0);
    assert_eq!(w.select_level(2).index.value(), 0);
    // Middle has 4 buckets: promotes past the coarsest.
    assert_eq!(w.select_level(3).index.value(), 1);
    assert_eq!(w.select_level(4).index.value(), 1);
    // Finest has 8 buckets.
    assert_eq!(w.select_level(5).index.value(), 2);
    assert_eq!(w.select_level(100).index.value(), 2);
}
