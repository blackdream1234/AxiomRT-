//! Ready queue abstraction (AXIOM-SCHED-001).
//!
//! Requirement reference: docs/09_SCHEDULER_MODEL.md §3.
//!
//! One FIFO ring per priority level, statically sized (no heap). FIFO
//! order within a level is the deterministic tie-breaking rule: among
//! equal priorities, the thread enqueued earliest is selected first.

use crate::thread::ThreadId;

use super::priority::{Priority, PRIORITY_LEVELS};

/// Capacity per priority level (static, no heap; v0.1 has a small fixed
/// task set).
pub const QUEUE_CAPACITY: usize = 16;

/// Explicit failure behavior for queue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// The level's ring is full (static capacity exhausted).
    Full,
    /// The thread is already enqueued (a thread queues at most once).
    AlreadyQueued,
}

/// Fixed-capacity FIFO ring of thread IDs.
#[derive(Debug, Clone, Copy)]
struct Fifo {
    slots: [Option<ThreadId>; QUEUE_CAPACITY],
    head: usize,
    len: usize,
}

impl Fifo {
    const fn new() -> Self {
        Fifo {
            slots: [None; QUEUE_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push_back(&mut self, tid: ThreadId) -> Result<(), QueueError> {
        if self.len == QUEUE_CAPACITY {
            return Err(QueueError::Full);
        }
        let tail = (self.head + self.len) % QUEUE_CAPACITY;
        self.slots[tail] = Some(tid);
        self.len += 1;
        Ok(())
    }

    fn pop_front(&mut self) -> Option<ThreadId> {
        if self.len == 0 {
            return None;
        }
        let tid = self.slots[self.head].take();
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.len -= 1;
        tid
    }

    fn contains(&self, tid: ThreadId) -> bool {
        (0..self.len).any(|i| self.slots[(self.head + i) % QUEUE_CAPACITY] == Some(tid))
    }

    /// Remove a specific thread, preserving FIFO order of the rest.
    fn remove(&mut self, tid: ThreadId) -> bool {
        let mut found = false;
        let mut kept = [None; QUEUE_CAPACITY];
        let mut kept_len = 0;
        for i in 0..self.len {
            let slot = self.slots[(self.head + i) % QUEUE_CAPACITY];
            if slot == Some(tid) && !found {
                found = true;
            } else {
                kept[kept_len] = slot;
                kept_len += 1;
            }
        }
        if found {
            self.slots = kept;
            self.head = 0;
            self.len = kept_len;
        }
        found
    }
}

/// Per-priority ready queues.
#[derive(Debug)]
pub struct ReadyQueue {
    levels: [Fifo; PRIORITY_LEVELS],
}

impl Default for ReadyQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadyQueue {
    pub const fn new() -> Self {
        ReadyQueue {
            levels: [Fifo::new(); PRIORITY_LEVELS],
        }
    }

    /// Enqueue a ready thread at its priority level (FIFO tail).
    /// A thread may be queued at most once across all levels.
    pub fn enqueue(&mut self, tid: ThreadId, prio: Priority) -> Result<(), QueueError> {
        if self.contains(tid) {
            return Err(QueueError::AlreadyQueued);
        }
        self.levels[prio.level() as usize].push_back(tid)
    }

    /// Pop the next thread: highest non-empty priority level, FIFO
    /// within the level. Deterministic by construction.
    pub fn pop_highest(&mut self) -> Option<(ThreadId, Priority)> {
        for level in (0..PRIORITY_LEVELS).rev() {
            if let Some(tid) = self.levels[level].pop_front() {
                let prio = Priority::new(level as u8).expect("level < PRIORITY_LEVELS");
                return Some((tid, prio));
            }
        }
        None
    }

    /// Remove a thread from wherever it is queued (kill/block/suspend
    /// path). Returns true if it was present.
    pub fn remove(&mut self, tid: ThreadId) -> bool {
        self.levels.iter_mut().any(|f| f.remove(tid))
    }

    pub fn contains(&self, tid: ThreadId) -> bool {
        self.levels.iter().any(|f| f.contains(tid))
    }

    pub fn is_empty(&self) -> bool {
        self.levels.iter().all(|f| f.len == 0)
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
    fn fifo_within_level() {
        let mut q = ReadyQueue::new();
        q.enqueue(tid(1), prio(3)).unwrap();
        q.enqueue(tid(2), prio(3)).unwrap();
        q.enqueue(tid(3), prio(3)).unwrap();
        assert_eq!(q.pop_highest().unwrap().0, tid(1));
        assert_eq!(q.pop_highest().unwrap().0, tid(2));
        assert_eq!(q.pop_highest().unwrap().0, tid(3));
        assert!(q.pop_highest().is_none());
    }

    #[test]
    fn higher_level_first() {
        let mut q = ReadyQueue::new();
        q.enqueue(tid(1), prio(1)).unwrap();
        q.enqueue(tid(2), prio(6)).unwrap();
        q.enqueue(tid(3), prio(4)).unwrap();
        assert_eq!(q.pop_highest().unwrap().0, tid(2));
        assert_eq!(q.pop_highest().unwrap().0, tid(3));
        assert_eq!(q.pop_highest().unwrap().0, tid(1));
    }

    #[test]
    fn double_enqueue_rejected() {
        let mut q = ReadyQueue::new();
        q.enqueue(tid(1), prio(2)).unwrap();
        assert_eq!(q.enqueue(tid(1), prio(5)), Err(QueueError::AlreadyQueued));
    }

    #[test]
    fn remove_preserves_order() {
        let mut q = ReadyQueue::new();
        for n in 1..=4 {
            q.enqueue(tid(n), prio(2)).unwrap();
        }
        assert!(q.remove(tid(2)));
        assert!(!q.remove(tid(2)), "second remove finds nothing");
        assert_eq!(q.pop_highest().unwrap().0, tid(1));
        assert_eq!(q.pop_highest().unwrap().0, tid(3));
        assert_eq!(q.pop_highest().unwrap().0, tid(4));
    }

    #[test]
    fn capacity_is_bounded() {
        let mut q = ReadyQueue::new();
        for n in 0..QUEUE_CAPACITY as u32 {
            q.enqueue(tid(n), prio(0)).unwrap();
        }
        assert_eq!(q.enqueue(tid(999), prio(0)), Err(QueueError::Full));
    }
}
