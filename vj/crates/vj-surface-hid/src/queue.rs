//! Bounded input event queue and key-coalescing output queue.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

use crate::{device::STREAM_DECK_XL_KEY_COUNT, ControlEvent};

/// Fixed-capacity input event queue.
/// Drops are tracked and counted to prevent memory unbounded growth under flood conditions.
pub struct BoundedInputQueue {
    capacity: usize,
    items: Mutex<VecDeque<ControlEvent>>,
    dropped: AtomicU64,
}

impl BoundedInputQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: Mutex::new(VecDeque::with_capacity(capacity)),
            dropped: AtomicU64::new(0),
        }
    }

    /// Push an event to the queue. If queue is saturated, drops the oldest event to ensure fresh real-time input.
    pub fn push(&self, event: ControlEvent) {
        let mut items = self.items.lock().unwrap();
        if items.len() >= self.capacity {
            items.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        items.push_back(event);
    }

    /// Pop the next event from the queue.
    pub fn pop(&self) -> Option<ControlEvent> {
        let mut items = self.items.lock().unwrap();
        items.pop_front()
    }

    /// Drain all queued events.
    pub fn drain(&self) -> Vec<ControlEvent> {
        let mut items = self.items.lock().unwrap();
        items.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        let items = self.items.lock().unwrap();
        items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        let mut items = self.items.lock().unwrap();
        items.clear();
    }
}

/// Bounded output queue that coalesces pending icon writes by key index.
///
/// Because Stream Deck XL has 32 physical keys, the pending write buffer is bounded to at most
/// 32 entries. Rapidly changing clip animations or show reloads will never grow the output queue
/// beyond 32 items.
pub struct CoalescingOutputQueue {
    inner: Mutex<OutputQueueInner>,
    coalesced: AtomicU64,
}

struct OutputQueueInner {
    pending: [Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT],
    order: VecDeque<u8>,
}

impl CoalescingOutputQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(OutputQueueInner {
                pending: Default::default(),
                order: VecDeque::with_capacity(STREAM_DECK_XL_KEY_COUNT),
            }),
            coalesced: AtomicU64::new(0),
        }
    }

    /// Push or coalesce an icon write for `key`.
    /// If key is already queued, updates its payload in-place and preserves order.
    pub fn push(&self, key: u8, jpeg_data: Vec<u8>) {
        if (key as usize) >= STREAM_DECK_XL_KEY_COUNT {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        let slot = &mut inner.pending[key as usize];

        if slot.is_some() {
            // Coalesce: overwrite existing pending payload
            *slot = Some(jpeg_data);
            self.coalesced.fetch_add(1, Ordering::Relaxed);
        } else {
            // Enqueue new key
            *slot = Some(jpeg_data);
            inner.order.push_back(key);
        }
    }

    /// Pop the next pending (key, jpeg_payload) to transmit.
    pub fn pop(&self) -> Option<(u8, Vec<u8>)> {
        let mut inner = self.inner.lock().unwrap();
        while let Some(key) = inner.order.pop_front() {
            if let Some(payload) = inner.pending[key as usize].take() {
                return Some((key, payload));
            }
        }
        None
    }

    /// Clear all pending output writes.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.pending = Default::default();
        inner.order.clear();
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn coalesced_count(&self) -> u64 {
        self.coalesced.load(Ordering::Relaxed)
    }
}

impl Default for CoalescingOutputQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_queue_bounded_and_drops_oldest() {
        let q = BoundedInputQueue::new(4);
        for i in 0..10 {
            q.push(ControlEvent::Press {
                key: i,
                logical_key: i,
                at_ns: i as u64,
            });
        }
        assert_eq!(q.len(), 4);
        assert_eq!(q.dropped_count(), 6);

        // Remaining events should be 6, 7, 8, 9
        let events = q.drain();
        assert_eq!(events.len(), 4);
        match &events[0] {
            ControlEvent::Press { key, .. } => assert_eq!(*key, 6),
            _ => panic!("wrong event"),
        }
    }

    #[test]
    fn output_queue_coalesces_same_key() {
        let q = CoalescingOutputQueue::new();

        // Push 5 updates for key 12
        for v in 1..=5 {
            q.push(12, vec![v]);
        }

        // Queue length must remain 1!
        assert_eq!(q.len(), 1);
        assert_eq!(q.coalesced_count(), 4);

        // Popping must yield the latest value (5)
        let (key, payload) = q.pop().unwrap();
        assert_eq!(key, 12);
        assert_eq!(payload, vec![5]);
        assert!(q.is_empty());
    }

    #[test]
    fn output_queue_bounds_to_32_keys() {
        let q = CoalescingOutputQueue::new();
        for k in 0..32u8 {
            q.push(k, vec![k]);
        }
        assert_eq!(q.len(), 32);

        // Pushing another 32 updates coalesces
        for k in 0..32u8 {
            q.push(k, vec![k + 100]);
        }
        assert_eq!(q.len(), 32);
        assert_eq!(q.coalesced_count(), 32);
    }
}
