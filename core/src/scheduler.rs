use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BinaryHeap};

/// A scheduler used by the emulation cores to schedule peripherals.
/// It is generic over the possible events and uses a binary heap
/// in combination with a circular u32 timer.
#[derive(Default, Deserialize, Serialize)]
pub struct Scheduler<E: Kind> {
    /// Current time of the scheduler.
    time: u32,
    /// Events currently awaiting execution.
    #[serde(bound = "")]
    events: BinaryHeap<ScheduledEvent<E>>,
}

impl<E: Kind> Scheduler<E> {
    /// Schedule an event of the given kind after the given amount
    /// of cycles have elapsed from now.
    pub fn schedule(&mut self, kind: E, after: u32) {
        self.events.push(ScheduledEvent {
            kind,
            execute_at: self.time + after,
        });
        // We run this here since it is probably the least-run function.
        // We want to check the time as litte as possible to save perf.
        self.check_time();
    }

    /// Advance the timer by the given amount of ticks.
    pub fn advance(&mut self, by: u32) {
        self.time = self.time.wrapping_add(by);
    }

    /// Get the next pending event awaiting execution. Returns None
    /// if all pending events have been processed.
    pub fn get_next_pending(&mut self) -> Option<Event<E>> {
        if self
            .events
            .peek()
            .is_some_and(|e| e.execute_at <= self.time)
        {
            let event = self.events.pop().unwrap();
            Some(Event {
                kind: event.kind,
                late_by: self.time - event.execute_at,
            })
        } else {
            None
        }
    }

    /// Checks to make sure the timer will not overflow by
    /// decrementing all times before that happens.
    #[inline]
    fn check_time(&mut self) {
        if self.time > 0xF000_0000 {
            self.time -= 0xF000_0000;
            // Sadly, BinaryHeap does not allow mutating the elements.
            // Since we know that this won't change order, it is safe
            for event in &self.events {
                let ptr = event as *const ScheduledEvent<E> as *mut ScheduledEvent<E>;
                unsafe {
                    (*ptr).execute_at -= 0xF000_0000;
                }
            }
        }
    }
}

/// An event awaiting execution
#[derive(Deserialize, Serialize)]
struct ScheduledEvent<E: Kind> {
    /// Kind of event to execute
    #[serde(bound = "")]
    kind: E,
    /// Time of the scheduler to execute it at
    execute_at: u32,
}

impl<E: Kind> PartialEq for ScheduledEvent<E> {
    fn eq(&self, other: &Self) -> bool {
        self.execute_at == other.execute_at
    }
}

impl<E: Kind> Eq for ScheduledEvent<E> {}

impl<E: Kind> PartialOrd for ScheduledEvent<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.execute_at.partial_cmp(&self.execute_at)
    }
    fn lt(&self, other: &Self) -> bool {
        other.execute_at < self.execute_at
    }
    fn le(&self, other: &Self) -> bool {
        other.execute_at <= self.execute_at
    }
    fn gt(&self, other: &Self) -> bool {
        other.execute_at > self.execute_at
    }
    fn ge(&self, other: &Self) -> bool {
        other.execute_at >= self.execute_at
    }
}

impl<E: Kind> Ord for ScheduledEvent<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.execute_at.cmp(&self.execute_at)
    }
}

/// Trait for event kinds.
pub trait Kind: for<'de> Deserialize<'de> + Serialize {}

/// Event that is ready to be handled.
pub struct Event<E: Kind> {
    /// The kind of event to handle
    pub kind: E,
    /// By how many ticks the event was delayed by. For example:
    /// - Event was scheduled to be executed at tick 1000
    /// - Scheduler ran until 1010
    /// - `late_by` will be 1010 - 1000 = 10.
    pub late_by: u32,
}
