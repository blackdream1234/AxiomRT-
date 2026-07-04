//! AxiomRT fixed-priority scheduler (Phase 6).
//!
//! Requirement reference: docs/09_SCHEDULER_MODEL.md,
//! docs/02_KERNEL_BLUEPRINT.md §10.
//!
//! Phase 6 scope: selection logic only. No timer preemption yet (the
//! timer tick calls into this model in a later phase), no context
//! switching. Pure logic — fully unit-tested on the host.

pub mod priority;
pub mod queue;

pub use priority::{Priority, PRIORITY_LEVELS};
pub use queue::{QueueError, ReadyQueue};

use crate::thread::ThreadId;

/// Fixed-priority scheduler (docs/09_SCHEDULER_MODEL.md).
///
/// Owns the ready queue. The kernel enqueues threads that become Ready
/// and removes threads that stop being Ready (blocked, killed, faulted,
/// suspended). Selection is total and deterministic:
/// highest priority level first, FIFO within a level.
///
/// Defense in depth: `select_next` takes an `is_ready` predicate (the
/// thread-state source of truth) and silently discards queue entries
/// that are no longer Ready. A killed or blocked thread is therefore
/// never selected even if a removal was missed — the queue is an
/// optimization, the state machine is the authority
/// (docs/09_SCHEDULER_MODEL.md §4, SCHED-P2/P3).
#[derive(Debug)]
pub struct FixedPriorityScheduler {
    ready: ReadyQueue,
}

impl FixedPriorityScheduler {
    pub const fn new() -> Self {
        FixedPriorityScheduler { ready: ReadyQueue::new() }
    }

    /// Make a thread eligible for selection at the given priority.
    pub fn mark_ready(&mut self, tid: ThreadId, prio: Priority) -> Result<(), QueueError> {
        self.ready.enqueue(tid, prio)
    }

    /// Remove a thread from eligibility (block/kill/fault/suspend).
    /// Returns true if it was queued.
    pub fn mark_not_ready(&mut self, tid: ThreadId) -> bool {
        self.ready.remove(tid)
    }

    /// Select the next thread to run: the highest-priority ready thread,
    /// FIFO among equals. Entries failing `is_ready` are discarded, not
    /// returned. Returns None if no ready thread exists (idle).
    pub fn select_next(
        &mut self,
        mut is_ready: impl FnMut(ThreadId) -> bool,
    ) -> Option<(ThreadId, Priority)> {
        while let Some((tid, prio)) = self.ready.pop_highest() {
            if is_ready(tid) {
                return Some((tid, prio));
            }
            // Stale entry (state changed without dequeue): drop it.
        }
        None
    }

    pub fn is_idle(&self) -> bool {
        self.ready.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid(n: u32) -> ThreadId {
        ThreadId(n)
    }
    fn prio(p: u8) -> Priority {
        Priority::new(p).unwrap()
    }

    #[test]
    fn selects_highest_priority_ready_thread() {
        let mut s = FixedPriorityScheduler::new();
        s.mark_ready(tid(1), prio(2)).unwrap();
        s.mark_ready(tid(2), prio(7)).unwrap();
        s.mark_ready(tid(3), prio(5)).unwrap();
        let (selected, p) = s.select_next(|_| true).unwrap();
        assert_eq!(selected, tid(2));
        assert_eq!(p, prio(7));
    }

    #[test]
    fn stale_not_ready_entries_are_never_selected() {
        let mut s = FixedPriorityScheduler::new();
        s.mark_ready(tid(1), prio(7)).unwrap();
        s.mark_ready(tid(2), prio(1)).unwrap();
        // tid 1 became not-ready without an explicit dequeue.
        let (selected, _) = s.select_next(|t| t != tid(1)).unwrap();
        assert_eq!(selected, tid(2));
    }

    #[test]
    fn idle_when_empty() {
        let mut s = FixedPriorityScheduler::new();
        assert!(s.is_idle());
        assert!(s.select_next(|_| true).is_none());
    }
}
