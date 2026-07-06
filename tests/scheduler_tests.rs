//! Scheduler integration tests (AXIOM-SCHED-002).
//!
//! Requirement reference: docs/09_SCHEDULER_MODEL.md §1/§4,
//! docs/14_TEST_STRATEGY.md.
//!
//! Runs on the host (no hardware dependency): drives the
//! FixedPriorityScheduler together with the Thread state machine, the
//! authority for readiness (docs/09 §4).

use kernel::memory::AddressSpaceId;
use kernel::sched::{FixedPriorityScheduler, Priority};
use kernel::thread::{Thread, ThreadId, ThreadState};

/// Small test harness: a static task table plus the scheduler, wired
/// the way the kernel will wire them (state machine = authority).
struct Harness {
    threads: Vec<Thread>,
    sched: FixedPriorityScheduler,
}

impl Harness {
    fn new(specs: &[(u32, u8)]) -> Self {
        let mut threads = Vec::new();
        let mut sched = FixedPriorityScheduler::new();
        for &(id, prio) in specs {
            let t = Thread::new(ThreadId(id), AddressSpaceId(id));
            sched
                .mark_ready(t.id(), Priority::new(prio).expect("valid test priority"))
                .expect("enqueue");
            threads.push(t);
        }
        Harness { threads, sched }
    }

    fn set_state(&mut self, id: u32, to: ThreadState) {
        let t = self
            .threads
            .iter_mut()
            .find(|t| t.id() == ThreadId(id))
            .expect("known thread");
        t.transition(to).expect("legal transition in test setup");
        if to != ThreadState::Ready {
            self.sched.mark_not_ready(ThreadId(id));
        }
    }

    /// Kernel-style selection: readiness comes from the state machine.
    fn select(&mut self) -> Option<ThreadId> {
        let threads = &self.threads;
        self.sched
            .select_next(|tid| {
                threads
                    .iter()
                    .find(|t| t.id() == tid)
                    .is_some_and(|t| t.state() == ThreadState::Ready)
            })
            .map(|(tid, _)| tid)
    }
}

#[test]
fn highest_priority_task_selected() {
    let mut h = Harness::new(&[(1, 2), (2, 7), (3, 5)]);
    assert_eq!(h.select(), Some(ThreadId(2)), "priority 7 beats 5 and 2");
}

#[test]
fn killed_task_not_selected() {
    let mut h = Harness::new(&[(1, 7), (2, 3)]);
    h.set_state(1, ThreadState::Killed);
    assert_eq!(
        h.select(),
        Some(ThreadId(2)),
        "killed high-prio task must be skipped"
    );
    assert_eq!(h.select(), None, "killed task never reappears");
}

#[test]
fn killed_task_not_selected_even_if_still_queued() {
    // Defense in depth (docs/09 §4): state changes without dequeue.
    let mut h = Harness::new(&[(1, 7), (2, 3)]);
    let t = h
        .threads
        .iter_mut()
        .find(|t| t.id() == ThreadId(1))
        .unwrap();
    t.transition(ThreadState::Killed).unwrap();
    // Deliberately NOT calling mark_not_ready(1): stale queue entry.
    assert_eq!(h.select(), Some(ThreadId(2)));
}

#[test]
fn blocked_task_not_selected() {
    let mut h = Harness::new(&[(1, 6), (2, 4)]);
    // Thread 1 starts running, then blocks in IPC.
    h.set_state(1, ThreadState::Running);
    h.set_state(1, ThreadState::Blocked);
    assert_eq!(
        h.select(),
        Some(ThreadId(2)),
        "blocked task must be skipped"
    );
}

#[test]
fn equal_priority_uses_deterministic_fifo_rule() {
    // Same priority: the earliest-ready thread wins, every time.
    let mut h = Harness::new(&[(10, 4), (11, 4), (12, 4)]);
    assert_eq!(h.select(), Some(ThreadId(10)));
    assert_eq!(h.select(), Some(ThreadId(11)));
    assert_eq!(h.select(), Some(ThreadId(12)));

    // Re-running the identical scenario yields the identical sequence
    // (SCHED-P3 determinism).
    let mut h2 = Harness::new(&[(10, 4), (11, 4), (12, 4)]);
    assert_eq!(h2.select(), Some(ThreadId(10)));
    assert_eq!(h2.select(), Some(ThreadId(11)));
    assert_eq!(h2.select(), Some(ThreadId(12)));
}

#[test]
fn faulted_task_not_selected() {
    let mut h = Harness::new(&[(1, 7), (2, 1)]);
    h.set_state(1, ThreadState::Running);
    h.set_state(1, ThreadState::Faulted);
    assert_eq!(
        h.select(),
        Some(ThreadId(2)),
        "faulted task cannot continue unless recovered"
    );
}
