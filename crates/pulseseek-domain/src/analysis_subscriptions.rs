use std::collections::{HashMap, HashSet};

use crate::analysis::{MeasurementPoint, SourceId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ChannelMode {
    Mono,
    Stereo,
    Left,
    Right,
    Mid,
    Side,
    EnergySum,
    LeftRightOverlay,
    LeftRightBalance,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WindowFunction {
    Hann,
    Hamming,
    BlackmanHarris,
    FlatTop,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProductKind {
    Fft,
    Spectrum,
    BandEnergy,
    Spectrogram,
    WaveformEnvelope,
    Loudness,
    TruePeak,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MailboxPolicy {
    LatestOnly,
    Continuous,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ValidityRequirement {
    Measured,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProductValidity {
    Unavailable,
    Measured,
    Complete,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProductKey {
    source_id: SourceId,
    source_point: MeasurementPoint,
    channel_mode: ChannelMode,
    fft_size: u32,
    window: WindowFunction,
    hop: u32,
    product_kind: ProductKind,
    algorithm_version: String,
    configuration_hash: u64,
}

impl ProductKey {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_id: SourceId,
        source_point: MeasurementPoint,
        channel_mode: ChannelMode,
        fft_size: u32,
        window: WindowFunction,
        hop: u32,
        product_kind: ProductKind,
        algorithm_version: impl Into<String>,
        configuration_hash: u64,
    ) -> Self {
        Self {
            source_id,
            source_point,
            channel_mode,
            fft_size,
            window,
            hop,
            product_kind,
            algorithm_version: algorithm_version.into(),
            configuration_hash,
        }
    }

    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    pub const fn source_point(&self) -> MeasurementPoint {
        self.source_point
    }
    pub const fn channel_mode(&self) -> ChannelMode {
        self.channel_mode
    }
    pub const fn fft_size(&self) -> u32 {
        self.fft_size
    }
    pub const fn window(&self) -> WindowFunction {
        self.window
    }
    pub const fn hop(&self) -> u32 {
        self.hop
    }
    pub const fn product_kind(&self) -> ProductKind {
        self.product_kind
    }
    pub fn algorithm_version(&self) -> &str {
        &self.algorithm_version
    }
    pub const fn configuration_hash(&self) -> u64 {
        self.configuration_hash
    }

    fn upstream(&self) -> Option<Self> {
        match self.product_kind {
            ProductKind::Spectrum
            | ProductKind::BandEnergy
            | ProductKind::Spectrogram
            | ProductKind::WaveformEnvelope => {
                Some(Self { product_kind: ProductKind::Fft, ..self.clone() })
            },
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionRequest {
    product_key: ProductKey,
    cadence_hz: u16,
    priority: u8,
    mailbox_policy: MailboxPolicy,
    validity_requirement: ValidityRequirement,
}

impl SubscriptionRequest {
    pub fn new(
        product_key: ProductKey,
        cadence_hz: u16,
        priority: u8,
        mailbox_policy: MailboxPolicy,
        validity_requirement: ValidityRequirement,
    ) -> Self {
        Self { product_key, cadence_hz, priority, mailbox_policy, validity_requirement }
    }

    pub fn product_key(&self) -> &ProductKey {
        &self.product_key
    }
    pub const fn cadence_hz(&self) -> u16 {
        self.cadence_hz
    }
    pub const fn priority(&self) -> u8 {
        self.priority
    }
    pub const fn mailbox_policy(&self) -> MailboxPolicy {
        self.mailbox_policy
    }
    pub const fn validity_requirement(&self) -> ValidityRequirement {
        self.validity_requirement
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    id: SubscriptionId,
    product_key: ProductKey,
    validity: ProductValidity,
}

impl Subscription {
    pub const fn id(&self) -> SubscriptionId {
        self.id
    }
    pub fn product_key(&self) -> &ProductKey {
        &self.product_key
    }
    pub const fn validity(&self) -> ProductValidity {
        self.validity
    }
}

#[derive(Clone, Debug)]
struct ProductNode {
    validity: ProductValidity,
    consumers: usize,
    dependencies: Vec<ProductKey>,
}

#[derive(Default)]
pub struct ProductGraph {
    next_subscription_id: u64,
    products: HashMap<ProductKey, ProductNode>,
    subscriptions: HashMap<SubscriptionId, ProductKey>,
}

impl ProductGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &mut self,
        request: SubscriptionRequest,
    ) -> Result<Subscription, SubscriptionError> {
        if request.product_key.fft_size == 0
            || request.product_key.hop == 0
            || request.cadence_hz == 0
        {
            return Err(SubscriptionError::InvalidRequest);
        }
        let mut pending = Vec::new();
        let mut key = request.product_key.clone();
        loop {
            pending.push(key.clone());
            match key.upstream() {
                Some(upstream) if !self.products.contains_key(&upstream) => key = upstream,
                _ => break,
            }
        }
        for product_key in pending.into_iter().rev() {
            let dependencies = product_key.upstream().into_iter().collect();
            self.products.entry(product_key).or_insert(ProductNode {
                validity: ProductValidity::Unavailable,
                consumers: 0,
                dependencies,
            });
        }
        self.next_subscription_id =
            self.next_subscription_id.checked_add(1).ok_or(SubscriptionError::IdExhausted)?;
        let id = SubscriptionId(self.next_subscription_id);
        let (dependencies, validity) = {
            let product = self
                .products
                .get_mut(&request.product_key)
                .ok_or(SubscriptionError::InvalidRequest)?;
            product.consumers += 1;
            (product.dependencies.clone(), product.validity)
        };
        for dependency in dependencies {
            self.products.get_mut(&dependency).expect("dependency registered").consumers += 1;
        }
        self.subscriptions.insert(id, request.product_key.clone());
        Ok(Subscription { id, product_key: request.product_key, validity })
    }

    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        let Some(key) = self.subscriptions.remove(&id) else { return false };
        self.release(&key);
        true
    }

    fn release(&mut self, key: &ProductKey) {
        let Some(node) = self.products.get_mut(key) else { return };
        node.consumers -= 1;
        let dependencies = node.dependencies.clone();
        let remove = node.consumers == 0;
        if remove {
            self.products.remove(key);
        }
        for dependency in dependencies {
            self.release(&dependency);
        }
    }

    pub fn consumer_count(&self, key: &ProductKey) -> usize {
        self.products.get(key).map_or(0, |node| node.consumers)
    }
    pub fn active_product_count(&self) -> usize {
        self.products.len()
    }
    pub fn current_validity(&self, key: &ProductKey) -> Option<ProductValidity> {
        self.products.get(key).map(|node| node.validity)
    }
    pub fn dependencies(&self, key: &ProductKey) -> Option<HashSet<ProductKey>> {
        self.products.get(key).map(|node| node.dependencies.iter().cloned().collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionError {
    InvalidRequest,
    IdExhausted,
}
