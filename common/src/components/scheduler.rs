// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use arrayvec::ArrayVec;

pub type Time = u64;
pub type TimeS = i64;

/// A scheduler used by the emulation cores to schedule peripherals.
/// It is generic over the possible events and uses a binary heap.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Scheduler<E: Kind> {
    /// Current time of the scheduler.
    time: Time,
    /// Time of the next event.
    next: Time,
    /// Events currently awaiting execution.
    #[cfg_attr(feature = "serde", serde(bound = ""))]
    events: ArrayVec<ScheduledEvent<E>, 16>,
}

impl<E: Kind> Scheduler<E> {
    /// Schedule an event of the given kind after the given amount
    /// of cycles have elapsed from now.
    /// Number can be negative; this is mainly used for events where
    /// they were quite a bit late and the followup event also needed to happen
    /// already.
    #[inline]
    pub fn schedule(&mut self, kind: E, after: TimeS) {
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
        self.next = self
            .events
            .last()
            .map(|e| e.execute_at)
            .unwrap_or(Time::MAX);
    }

    /// Advance the timer by the given amount of ticks.
    #[inline]
    pub fn advance(&mut self, by: Time) {
        self.time += by;
    }

    /// Execute all pending events in order with the given closure.
    /// Note that this implementation assumes there is always at least one event
    /// scheduled.
    #[inline]
    pub fn get_next_pending(&mut self) -> Option<Event<E>> {
        if self.next <= self.time {
            let idx = self.events.len() - 1;
            let event = self.events[idx];
            unsafe { self.events.set_len(idx) };
            self.next = self
                .events
                .last()
                .map(|e| e.execute_at)
                .unwrap_or(Time::MAX);
            Some(Event {
                kind: event.kind,
                late_by: (self.time - event.execute_at) as TimeS,
            })
        } else {
            None
        }
    }

    #[inline]
    pub fn has_events(&self) -> bool {
        self.next <= self.time
    }

    /// Return the next event immediately, and set the current time to
    /// the event's execution time. This is useful during HALT or similar
    /// states.
    pub fn pop(&mut self) -> Event<E> {
        let event = self.events.pop().unwrap();
        self.time = event.execute_at;
        self.next = self
            .events
            .last()
            .map(|e| e.execute_at)
            .unwrap_or(Time::MAX);
        Event {
            kind: event.kind,
            late_by: 0,
        }
    }

    /// Cancel all events of a given type.
    /// Somewhat expensive.
    pub fn cancel(&mut self, evt: E) {
        self.events.retain(|e| e.kind != evt);
        self.next = self
            .events
            .last()
            .map(|e| e.execute_at)
            .unwrap_or(Time::MAX);
    }

    /// Cancel an event of a given type.
    /// Somewhat less expensive than `cancel`.
    pub fn cancel_single(&mut self, evt: E) -> bool {
        let idx = self.events.iter().position(|e| e.kind == evt);
        if let Some(idx) = idx {
            self.events.remove(idx);
            self.next = self
                .events
                .last()
                .map(|e| e.execute_at)
                .unwrap_or(Time::MAX);
        }
        idx.is_some()
    }

    /// Cancel a single (!) matching event and return it's remaining time.
    pub fn cancel_with_remaining(&mut self, mut evt: impl FnMut(E) -> bool) -> (Time, E) {
        let idx = self.events.iter().position(|e| evt(e.kind)).unwrap();
        let evt = self.events.remove(idx);
        self.next = self
            .events
            .last()
            .map(|e| e.execute_at)
            .unwrap_or(Time::MAX);
        (evt.execute_at - self.time, evt.kind)
    }

    #[inline]
    pub fn now(&self) -> Time {
        self.time
    }
}

/// An event awaiting execution
#[derive(Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct ScheduledEvent<E: Kind> {
    /// Kind of event to execute
    #[cfg_attr(feature = "serde", serde(bound = ""))]
    kind: E,
    /// Time of the scheduler to execute it at
    execute_at: Time,
}

/// Trait for event kinds.
#[cfg(feature = "serde")]
pub trait Kind:
    for<'de> serde::Deserialize<'de> + serde::Serialize + PartialEq + Copy + Clone
{
}
#[cfg(not(feature = "serde"))]
pub trait Kind: PartialEq + Copy + Clone {}

/// Event that is ready to be handled.
#[derive(Copy, Clone)]
pub struct Event<E: Kind> {
    /// The kind of event to handle
    pub kind: E,
    /// By how many ticks the event was delayed by. For example:
    /// - Event was scheduled to be executed at tick 1000
    /// - Scheduler ran until 1010 before the event got handled
    /// - `late_by` will be 1010 - 1000 = 10.
    pub late_by: TimeS,
}
