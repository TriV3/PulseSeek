mod support;

use pulseseek_decoder_symphonia::WavDecoder;
use pulseseek_domain::decoder::Decoder;
use support::calibration::{
    catalogue, crc32, generate, wav_bytes, ChannelVariant, FixtureId, GENERATOR_VERSION,
    SAMPLE_RATES,
};

#[test]
fn catalogue_covers_required_fixture_families_rates_and_layouts() {
    let fixtures = catalogue();

    for id in FixtureId::ALL {
        assert!(fixtures.iter().any(|fixture| fixture.id == id), "missing {id:?}");
    }
    for sample_rate in SAMPLE_RATES {
        for (id, variants) in [
            (FixtureId::F001, &[ChannelVariant::Mono, ChannelVariant::Stereo][..]),
            (FixtureId::F002, &[ChannelVariant::Mono, ChannelVariant::StereoIdentical][..]),
            (FixtureId::F003, &[ChannelVariant::Mono][..]),
            (FixtureId::F004, &[ChannelVariant::Mono][..]),
            (FixtureId::F005, &[ChannelVariant::Mono][..]),
            (FixtureId::F006, &[ChannelVariant::StereoIdentical][..]),
            (FixtureId::F007, &[ChannelVariant::StereoInverted][..]),
            (FixtureId::F008, &[ChannelVariant::LeftOnly][..]),
            (FixtureId::F009, &[ChannelVariant::RightOnly][..]),
            (FixtureId::F010, &[ChannelVariant::Mono, ChannelVariant::StereoIdentical][..]),
            (FixtureId::F011, &[ChannelVariant::Mono, ChannelVariant::StereoIdentical][..]),
        ] {
            for variant in variants {
                assert!(
                    fixtures.iter().any(|fixture| {
                        fixture.id == id
                            && fixture.sample_rate == sample_rate
                            && fixture.channels == *variant
                    }),
                    "missing {id:?} at {sample_rate} Hz with {variant:?}"
                );
            }
        }
    }
}

#[test]
fn fixtures_have_complete_versioned_metadata() {
    for specification in catalogue() {
        let fixture = generate(&specification).unwrap();

        assert_eq!(fixture.metadata.generator_version, GENERATOR_VERSION);
        assert_eq!(
            fixture.metadata.frame_count as usize * fixture.metadata.channels as usize,
            fixture.samples.len()
        );
        assert!(fixture.metadata.duration_seconds > 0.0);
        assert!(!fixture.metadata.expected_result.is_empty());
        assert!(!fixture.metadata.tolerance.is_empty());
        assert!(!fixture.metadata.provenance.is_empty());
        assert!(!fixture.metadata.license.is_empty());
        assert_eq!(fixture.metadata.checksum, fixture.checksum());
    }
}

#[test]
fn generation_is_repeatable_and_channel_variants_are_exact() {
    for specification in catalogue() {
        let first = generate(&specification).unwrap();
        let second = generate(&specification).unwrap();
        assert_eq!(first.samples, second.samples);
        assert_eq!(first.metadata.checksum, second.metadata.checksum);

        for frame in first.samples.chunks_exact(first.metadata.channels as usize) {
            match specification.channels {
                ChannelVariant::StereoIdentical => assert_eq!(frame[0], frame[1]),
                ChannelVariant::StereoInverted => assert_eq!(frame[0], -frame[1]),
                ChannelVariant::LeftOnly => assert_eq!(frame[1], 0.0),
                ChannelVariant::RightOnly => assert_eq!(frame[0], 0.0),
                ChannelVariant::Mono | ChannelVariant::Stereo => {},
            }
        }
    }
}

#[test]
fn phase_fixtures_have_expected_correlation_and_mono_cancellation() {
    for id in [FixtureId::F006, FixtureId::F007] {
        for sample_rate in SAMPLE_RATES {
            let specification = catalogue()
                .into_iter()
                .find(|fixture| fixture.id == id && fixture.sample_rate == sample_rate)
                .unwrap();
            let fixture = generate(&specification).unwrap();
            let (dot, left_power, right_power, mono_power) = fixture.samples.chunks_exact(2).fold(
                (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64),
                |values, frame| {
                    let left = f64::from(frame[0]);
                    let right = f64::from(frame[1]);
                    (
                        values.0 + left * right,
                        values.1 + left * left,
                        values.2 + right * right,
                        values.3 + ((left + right) * 0.5).powi(2),
                    )
                },
            );
            let correlation = dot / (left_power * right_power).sqrt();

            if id == FixtureId::F006 {
                assert!((correlation - 1.0).abs() < f64::EPSILON);
                assert!(mono_power > 0.0);
            } else {
                assert!((correlation + 1.0).abs() < f64::EPSILON);
                assert_eq!(mono_power, 0.0);
            }
        }
    }
}

#[test]
fn calibrated_signals_have_expected_properties() {
    let fixtures = catalogue();
    let silence =
        generate(fixtures.iter().find(|fixture| fixture.id == FixtureId::F001).unwrap()).unwrap();
    assert!(silence.samples.iter().all(|sample| *sample == 0.0));

    for id in [FixtureId::F002, FixtureId::F003, FixtureId::F004, FixtureId::F005] {
        for sample_rate in SAMPLE_RATES {
            let specification = fixtures
                .iter()
                .find(|fixture| fixture.id == id && fixture.sample_rate == sample_rate)
                .unwrap();
            let fixture = generate(specification).unwrap();
            let peak = fixture.samples.iter().copied().map(f32::abs).fold(0.0, f32::max);
            assert!(peak > 0.0 && peak <= 1.0);
            if id == FixtureId::F002 || id == FixtureId::F003 || id == FixtureId::F005 {
                assert!((peak - 10_f32.powf(-18.0 / 20.0)).abs() < 0.000_01);
            }
        }
    }

    let impulse =
        generate(fixtures.iter().find(|fixture| fixture.id == FixtureId::F010).unwrap()).unwrap();
    assert_eq!(
        impulse.samples.iter().filter(|sample| **sample != 0.0).count(),
        impulse.metadata.channels as usize
    );

    let isp =
        generate(fixtures.iter().find(|fixture| fixture.id == FixtureId::F011).unwrap()).unwrap();
    let sampled_peak = isp.samples.iter().copied().map(f32::abs).fold(0.0, f32::max);
    assert!((sampled_peak - 0.99 / 2_f32.sqrt()).abs() < 0.000_001);
    assert!(isp.metadata.expected_result.contains("-0.0873 dBTP"));
}

#[test]
fn checksums_are_standard_stable_and_sensitive() {
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    let specification = catalogue()
        .into_iter()
        .find(|fixture| {
            fixture.id == FixtureId::F011
                && fixture.sample_rate == 48_000
                && fixture.channels == ChannelVariant::Mono
        })
        .unwrap();
    let fixture = generate(&specification).unwrap();
    let repeated = generate(&specification).unwrap();
    assert_eq!(fixture.metadata.checksum, repeated.metadata.checksum);
    assert_eq!(fixture.metadata.checksum, 878_710_213);
    let mut sample_mutated = fixture.clone();
    sample_mutated.samples[0] = f32::from_bits(sample_mutated.samples[0].to_bits() ^ 1);
    assert_ne!(fixture.metadata.checksum, sample_mutated.checksum());

    let mut metadata_mutated = fixture.clone();
    metadata_mutated.metadata.expected_result = "changed expectation";
    assert_eq!(fixture.samples, metadata_mutated.samples);
    assert_ne!(fixture.metadata.checksum, metadata_mutated.checksum());

    let other_rate = catalogue()
        .into_iter()
        .find(|fixture| {
            fixture.id == FixtureId::F002
                && fixture.sample_rate == 96_000
                && fixture.channels == ChannelVariant::Mono
        })
        .unwrap();
    assert_ne!(fixture.metadata.checksum, generate(&other_rate).unwrap().metadata.checksum);
}

#[test]
fn wav_encoding_is_deterministic_and_structurally_valid() {
    for channels in [ChannelVariant::Mono, ChannelVariant::StereoIdentical] {
        let specification = catalogue()
            .into_iter()
            .find(|fixture| {
                fixture.id == FixtureId::F002
                    && fixture.sample_rate == 48_000
                    && fixture.channels == channels
            })
            .unwrap();
        let fixture = generate(&specification).unwrap();
        let first = wav_bytes(&fixture);
        let second = wav_bytes(&fixture);

        assert_eq!(first, second);
        assert_eq!(&first[0..4], b"RIFF");
        assert_eq!(&first[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([first[20], first[21]]), 1);
        assert_eq!(u16::from_le_bytes([first[22], first[23]]), channels.channel_count());
        assert_eq!(u32::from_le_bytes(first[24..28].try_into().unwrap()), 48_000);
        assert_eq!(u16::from_le_bytes([first[32], first[33]]), channels.channel_count() * 3);
        assert_eq!(u16::from_le_bytes([first[34], first[35]]), 24);
        assert_eq!(first.len(), 44 + fixture.samples.len() * 3);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calibration.wav");
        std::fs::write(&path, &first).unwrap();
        let mut decoder = WavDecoder::open(&path).unwrap();
        let metadata = decoder.metadata().unwrap();
        assert_eq!(metadata.sample_rate, 48_000);
        assert_eq!(metadata.channels, channels.channel_count());
        let mut decoded = vec![0.0; fixture.samples.len()];
        let decoded_count = decoder.read(&mut decoded).unwrap();
        assert!(decoded_count > 0);
    }
}
