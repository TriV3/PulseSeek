use pulseseek_domain::analysis_events::{DeliveryPolicy, EventFamily, EventValidity};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelope {
    family: EventFamily,
    sequence: u64,
    timestamp_samples: u64,
    cadence_hz: u16,
    validity: EventValidity,
}
impl EventEnvelope {
    pub fn new(
        family: EventFamily,
        sequence: u64,
        timestamp_samples: u64,
        validity: EventValidity,
    ) -> Result<Self, &'static str> {
        Ok(Self { family, sequence, timestamp_samples, cadence_hz: 0, validity })
    }
    pub const fn family(&self) -> EventFamily {
        self.family
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn timestamp_samples(&self) -> u64 {
        self.timestamp_samples
    }
    pub const fn validity(&self) -> EventValidity {
        self.validity
    }
    pub const fn cadence_hz(&self) -> u16 {
        self.cadence_hz
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeteringPublishResult {
    Accepted,
    Dropped,
    Gap,
    ReceiverGone,
    CadenceRejected,
}
impl MeteringPublishResult {
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
    pub const fn is_dropped(self) -> bool {
        matches!(self, Self::Dropped)
    }
    pub const fn is_gap(self) -> bool {
        matches!(self, Self::Gap)
    }
}

struct Slot {
    capacity: usize,
    queue: VecDeque<EventEnvelope>,
    receivers: usize,
    validity: EventValidity,
}
pub struct EventReceiver {
    runtime: Arc<Mutex<HashMap<EventFamily, Slot>>>,
    subscriptions: Arc<Mutex<HashMap<u64, EventFamily>>>,
    family: EventFamily,
    id: u64,
    active: bool,
}
impl EventReceiver {
    pub const fn id(&self) -> u64 {
        self.id
    }
    pub fn unsubscribe(mut self) -> bool {
        if !self.active {
            return false;
        }
        let removed = self.runtime_owner_unsubscribe();
        self.active = false;
        removed
    }
    fn runtime_owner_unsubscribe(&self) -> bool {
        let Some(family) = self
            .subscriptions
            .lock()
            .ok()
            .and_then(|mut subscriptions| subscriptions.remove(&self.id))
        else {
            return false;
        };
        if let Ok(mut slots) = self.runtime.lock() {
            if let Some(slot) = slots.get_mut(&family) {
                slot.receivers = slot.receivers.saturating_sub(1);
                if slot.receivers == 0 {
                    slot.queue.clear();
                }
            }
        }
        true
    }
    pub fn try_receive(&self) -> Option<EventEnvelope> {
        self.runtime.lock().ok()?.get_mut(&self.family)?.queue.pop_front()
    }
}
impl Drop for EventReceiver {
    fn drop(&mut self) {
        let removed = self.active
            && self
                .subscriptions
                .lock()
                .ok()
                .and_then(|mut subscriptions| subscriptions.remove(&self.id))
                .is_some();
        if !removed {
            return;
        }
        if let Ok(mut slots) = self.runtime.lock() {
            if let Some(slot) = slots.get_mut(&self.family) {
                slot.receivers = slot.receivers.saturating_sub(1);
                if slot.receivers == 0 {
                    slot.queue.clear();
                }
            }
        }
    }
}

pub struct AnalysisEventRuntime {
    slots: Arc<Mutex<HashMap<EventFamily, Slot>>>,
    subscriptions: Arc<Mutex<HashMap<u64, EventFamily>>>,
    next_id: AtomicU64,
}
impl AnalysisEventRuntime {
    pub fn new(default_capacity: usize) -> Self {
        let slots = EventFamily::ALL
            .into_iter()
            .map(|family| {
                (
                    family,
                    Slot {
                        capacity: default_capacity.max(1),
                        queue: VecDeque::new(),
                        receivers: 0,
                        validity: EventValidity::Measured,
                    },
                )
            })
            .collect();
        Self {
            slots: Arc::new(Mutex::new(slots)),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(0),
        }
    }
    pub fn subscribe(&self, family: EventFamily, capacity: usize) -> Option<EventReceiver> {
        let mut slots = self.slots.lock().ok()?;
        let slot = slots.get_mut(&family)?;
        slot.capacity = capacity.max(1);
        slot.receivers += 1;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        drop(slots);
        self.subscriptions.lock().ok()?.insert(id, family);
        Some(EventReceiver {
            runtime: Arc::clone(&self.slots),
            subscriptions: Arc::clone(&self.subscriptions),
            family,
            id,
            active: true,
        })
    }
    pub fn publish_at(&self, event: EventEnvelope, cadence_hz: u16) -> MeteringPublishResult {
        if let DeliveryPolicy::Cadenced { min_hz, max_hz } = event.family.policy() {
            if !(min_hz..=max_hz).contains(&cadence_hz) {
                return MeteringPublishResult::CadenceRejected;
            }
        }
        self.publish(event)
    }
    pub fn publish(&self, event: EventEnvelope) -> MeteringPublishResult {
        let Ok(mut slots) = self.slots.lock() else { return MeteringPublishResult::ReceiverGone };
        let Some(slot) = slots.get_mut(&event.family) else {
            return MeteringPublishResult::ReceiverGone;
        };
        if slot.receivers == 0 {
            return MeteringPublishResult::ReceiverGone;
        }
        if slot.queue.len() >= slot.capacity {
            match event.family.policy() {
                DeliveryPolicy::LatestOnly => {
                    slot.queue.pop_front();
                    slot.queue.push_back(event);
                    MeteringPublishResult::Dropped
                },
                DeliveryPolicy::ContinuousAndDisplay => {
                    slot.validity = EventValidity::Incomplete;
                    MeteringPublishResult::Gap
                },
                _ => MeteringPublishResult::Dropped,
            }
        } else {
            slot.queue.push_back(event);
            MeteringPublishResult::Accepted
        }
    }
    pub fn family_validity(&self, family: EventFamily) -> EventValidity {
        self.slots
            .lock()
            .ok()
            .and_then(|slots| slots.get(&family).map(|slot| slot.validity))
            .unwrap_or(EventValidity::Unavailable)
    }
    pub fn unsubscribe(&self, id: u64) -> bool {
        let Some(family) =
            self.subscriptions.lock().ok().and_then(|mut subscriptions| subscriptions.remove(&id))
        else {
            return false;
        };
        if let Ok(mut slots) = self.slots.lock() {
            if let Some(slot) = slots.get_mut(&family) {
                slot.receivers = slot.receivers.saturating_sub(1);
                if slot.receivers == 0 {
                    slot.queue.clear();
                }
            }
        }
        true
    }
}
