// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};

/// A scheduler used by the emulation cores to schedule peripherals.
/// It is generic over the possible events and uses a binary heap
/// in combination with a circular u32 timer.
#[derive(Default, Deserialize, Serialize)]
pub struct Scheduler<E: Kind> {
    /// Current time of the scheduler.
    time: u32,
    /// Time of the next event.
    next: u32,
    /// Events currently awaiting execution.
    #[serde(bound = "")]
    events: ArrayVec<ScheduledEvent<E>, 16>,
}

impl<E: Kind> Scheduler<E> {
    /// Schedule an event of the given kind after the given amount
    /// of cycles have elapsed from now.
    /// Number can be negative; this is mainly used for events where
    /// they were quite a bit late and the followup event also needed to happen
    /// already.
    pub fn schedule(&mut self, kind: E, after: i32) {
        let time = self.time.saturating_add_signed(after);
        let event = ScheduledEvent {
            kind,
            execute_at: time,
        };
        unsafe { self.events.push_unchecked(event) };

        // Ensure the event list is still sorted
        // (Swap the new element further back until it is in the right spot)
        // I tried multiple implementations (using Vec::swap, Vec::insert)
        // and this was the fastest.
        for idx in (1..self.events.len()).rev() {
            let other = self.events[idx - 1];
            if time > other.execute_at {
                self.events[idx] = other;
            } else {
                self.events[idx] = event;
                self.next = self.events.last().unwrap().execute_at;
                return;
            }
        }
        // The loop exited without finding a bigger element, this new one is the biggest
        self.events[0] = event;
        self.next = self.events.last().unwrap().execute_at;

        // We run this here since it is probably the least-run function.
        // We want to check the time as little as possible to save perf.
        self.check_time();
    }

    /// Advance the timer by the given amount of ticks.
    #[inline]
    pub fn advance(&mut self, by: u32) {
        self.time += by;
    }

    /// Execute all pending events in order with the given closure.
    /// Note that this implementation assumes there is always at least one event
    /// scheduled.
    pub fn get_next_pending(&mut self) -> Option<Event<E>> {
        if self.next <= self.time {
            let idx = self.events.len() - 1;
            let event = self.events[idx];
            unsafe { self.events.set_len(idx) };
            self.next = self.events.last().unwrap().execute_at;
            Some(Event {
                kind: event.kind,
                late_by: (self.time - event.execute_at) as i32,
            })
        } else {
            None
        }
    }

    pub fn has_events(&self) -> bool {
        self.next <= self.time
    }

    /// Return the next event immediately, and set the current time to
    /// the event's execution time. This is useful during HALT or similar
    /// states.
    pub fn pop(&mut self) -> Event<E> {
        let event = self.events.pop().unwrap();
        self.time = event.execute_at;
        self.next = self.events.last().unwrap().execute_at;
        Event {
            kind: event.kind,
            late_by: 0,
        }
    }

    /// Cancel all events of a given type.
    /// Somewhat expensive.
    pub fn cancel(&mut self, evt: E) {
        self.events.retain(|e| e.kind != evt);
        self.next = self.events.last().unwrap().execute_at;
    }

    pub fn now(&self) -> u32 {
        self.time
    }

    /// Checks to make sure the timer will not overflow by
    /// decrementing all times before that happens.
    #[inline]
    fn check_time(&mut self) {
        if self.time > 0xF000_0000 {
            self.time -= 0xF000_0000;
            for event in &mut self.events {
                event.execute_at -= 0xF000_0000;
            }
        }
    }
}

/// An event awaiting execution
#[derive(Copy, Clone, Deserialize, Serialize)]
struct ScheduledEvent<E: Kind> {
    /// Kind of event to execute
    #[serde(bound = "")]
    kind: E,
    /// Time of the scheduler to execute it at
    execute_at: u32,
}

/// Trait for event kinds.
pub trait Kind: for<'de> Deserialize<'de> + Serialize + PartialEq + Copy + Clone {}

/// Event that is ready to be handled.
#[derive(Copy, Clone)]
pub struct Event<E: Kind> {
    /// The kind of event to handle
    pub kind: E,
    /// By how many ticks the event was delayed by. For example:
    /// - Event was scheduled to be executed at tick 1000
    /// - Scheduler ran until 1010 before the event got handled
    /// - `late_by` will be 1010 - 1000 = 10.
    pub late_by: i32,
}
