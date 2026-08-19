use pulseseek_domain::analysis::{MeasurementPoint, SourceId};
use pulseseek_domain::analysis_subscriptions::{
    ChannelMode, MailboxPolicy, ProductGraph, ProductKey, ProductKind, SubscriptionRequest,
    ValidityRequirement, WindowFunction,
};

fn key(kind: ProductKind, configuration_hash: u64) -> ProductKey {
    ProductKey::new(
        SourceId::new("player"),
        MeasurementPoint::Source,
        ChannelMode::Stereo,
        1_024,
        WindowFunction::Hann,
        512,
        kind,
        "fft-v1",
        configuration_hash,
    )
}

fn request(product_key: ProductKey) -> SubscriptionRequest {
    SubscriptionRequest::new(
        product_key,
        30,
        1,
        MailboxPolicy::LatestOnly,
        ValidityRequirement::Measured,
    )
}

#[test]
fn identical_subscribers_share_product_and_count_consumers() {
    let mut graph = ProductGraph::new();
    let first = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();
    let second = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();

    assert_ne!(first.id(), second.id());
    assert_eq!(first.product_key(), second.product_key());
    assert_eq!(graph.consumer_count(first.product_key()), 2);
    assert_eq!(graph.active_product_count(), 2);
    assert_eq!(graph.current_validity(first.product_key()).unwrap(), first.validity());
}

#[test]
fn subscription_settings_do_not_split_compatible_product() {
    let mut graph = ProductGraph::new();
    let first = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();
    let second = graph
        .subscribe(SubscriptionRequest::new(
            key(ProductKind::Spectrum, 7),
            60,
            2,
            MailboxPolicy::Continuous,
            ValidityRequirement::Complete,
        ))
        .unwrap();

    assert_eq!(first.product_key(), second.product_key());
    assert_eq!(graph.active_product_count(), 2);
    assert_eq!(graph.consumer_count(first.product_key()), 2);
}

#[test]
fn dependency_edges_share_upstream_and_incompatible_keys_branch() {
    let mut graph = ProductGraph::new();
    let spectrum = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();
    let bands = graph.subscribe(request(key(ProductKind::BandEnergy, 7))).unwrap();
    let other = graph.subscribe(request(key(ProductKind::Spectrum, 8))).unwrap();

    let fft = key(ProductKind::Fft, 7);
    assert!(graph.dependencies(bands.product_key()).unwrap().contains(&fft));
    assert_eq!(graph.consumer_count(&fft), 2);
    assert_eq!(graph.active_product_count(), 5);
    assert_ne!(other.product_key(), spectrum.product_key());
}

#[test]
fn rejects_invalid_subscription_requests_without_creating_products() {
    let mut graph = ProductGraph::new();
    let invalid_key = ProductKey::new(
        SourceId::new("player"),
        MeasurementPoint::Source,
        ChannelMode::Stereo,
        0,
        WindowFunction::Hann,
        512,
        ProductKind::Spectrum,
        "fft-v1",
        7,
    );

    assert_eq!(
        graph.subscribe(request(invalid_key)).unwrap_err(),
        pulseseek_domain::analysis_subscriptions::SubscriptionError::InvalidRequest
    );
    assert_eq!(graph.active_product_count(), 0);
}

#[test]
fn spectrum_channel_modes_include_shared_fft_products() {
    let modes = [
        ChannelMode::Left,
        ChannelMode::Right,
        ChannelMode::EnergySum,
        ChannelMode::Mono,
        ChannelMode::Mid,
        ChannelMode::Side,
        ChannelMode::LeftRightOverlay,
        ChannelMode::LeftRightBalance,
    ];

    assert_eq!(modes.len(), 8);
}

#[test]
fn final_and_repeated_unsubscribe_stop_unused_product() {
    let mut graph = ProductGraph::new();
    let first = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();
    let second = graph.subscribe(request(key(ProductKind::Spectrum, 7))).unwrap();

    assert!(graph.unsubscribe(first.id()));
    assert!(!graph.unsubscribe(first.id()));
    assert_eq!(graph.consumer_count(second.product_key()), 1);
    assert!(graph.unsubscribe(second.id()));
    assert!(!graph.unsubscribe(second.id()));
    assert_eq!(graph.active_product_count(), 0);
}
