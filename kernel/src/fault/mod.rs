//! AxiomRT fault handling (Phase 10).
//!
//! Requirement reference: docs/06_FAULT_MODEL.md.
//!
//! AXIOM-FAULT-001: the structured FaultEvent model.
//! AXIOM-FAULT-002: the basic handling policy — a user fault is
//! *contained* (thread marked Faulted, kernel unaffected); a fault in
//! kernel scope is a KernelInvariantViolation and halts safely.
//! AXIOM-FAULT-003 adds the supervisor notification path.

pub mod event;
pub mod wire;

pub use event::{EventState, FaultEvent, FaultType, IllegalEventTransition, Severity};
pub use wire::{
    acknowledge, decode, default_decision, encode, is_valid_recovery, notify_supervisor,
    AckError, DecodeError, FaultReport, NotifyOutcome, RecoveryDecision, KERNEL_SENDER,
};

use crate::thread::{Thread, ThreadState};

/// Where a fault occurred (docs/06: kernel faults and user faults are
/// treated differently).
pub enum FaultScope<'a> {
    /// A user task faulted; the thread is contained.
    User(&'a mut Thread),
    /// The kernel itself detected the fault while running kernel code.
    Kernel,
}

/// Total outcome of fault handling (AXIOM-FAULT-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultOutcome {
    /// User fault contained: the thread is Faulted and will never run
    /// again as-is; the kernel continues. The event must now reach the
    /// supervisor (AXIOM-FAULT-003).
    Contained(FaultEvent),
    /// Kernel integrity can no longer be trusted: the caller must halt
    /// safely (docs/06, KernelPanic). Never returned for a well-formed
    /// user fault.
    KernelPanic(FaultEvent),
}

/// Handle a fault. Total function: every (scope, fault type, thread
/// state) combination has exactly one defined outcome — there is no
/// undefined fault behavior (docs/06 definition of done).
///
/// Rules (docs/06_FAULT_MODEL.md):
/// * Kernel scope → KernelPanic, always; the event is forced to
///   KernelInvariantViolation/Fatal (a kernel-context page fault *is*
///   an invariant violation, docs/05 §8).
/// * User scope + KernelInvariantViolation → KernelPanic (a user task
///   cannot legitimately raise a kernel invariant fault; seeing one
///   means kernel state is inconsistent).
/// * User scope, live thread → thread moves to Faulted, outcome
///   Contained. Containment touches only the faulting thread.
/// * User scope, thread not in a live state (already Killed/Faulted/
///   Suspended) → KernelPanic: the kernel attempted to fault a thread
///   that cannot fault, which is itself an inconsistency.
pub fn handle(event_id: u64, fault_type: FaultType, scope: FaultScope<'_>, pc: u64, detail: u64) -> FaultOutcome {
    match scope {
        FaultScope::Kernel => {
            let ev = FaultEvent::new(
                event_id,
                FaultType::KernelInvariantViolation,
                crate::thread::ThreadId(0),
                pc,
                detail,
            );
            FaultOutcome::KernelPanic(ev)
        }
        FaultScope::User(thread) => {
            if fault_type == FaultType::KernelInvariantViolation {
                let ev = FaultEvent::new(event_id, fault_type, thread.id(), pc, detail);
                return FaultOutcome::KernelPanic(ev);
            }
            let ev = FaultEvent::new(event_id, fault_type, thread.id(), pc, detail);
            match thread.transition(ThreadState::Faulted) {
                Ok(()) => FaultOutcome::Contained(ev),
                Err(_) => FaultOutcome::KernelPanic(ev),
            }
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use crate::memory::AddressSpaceId;
    use crate::sched::{FixedPriorityScheduler, Priority};
    use crate::thread::ThreadId;

    fn live_thread(id: u32) -> Thread {
        Thread::new(ThreadId(id), AddressSpaceId(id))
    }

    #[test]
    fn user_fault_is_contained_and_marks_thread_faulted() {
        let mut t = live_thread(5);
        t.transition(ThreadState::Running).unwrap();
        let out = handle(1, FaultType::IllegalInstruction, FaultScope::User(&mut t), 0x1000, 0);
        match out {
            FaultOutcome::Contained(ev) => {
                assert_eq!(ev.thread(), ThreadId(5));
                assert_eq!(ev.fault_type(), FaultType::IllegalInstruction);
                assert_eq!(ev.severity(), Severity::Error);
            }
            other => panic!("expected Contained, got {other:?}"),
        }
        assert_eq!(t.state(), ThreadState::Faulted, "user-space fault does not crash kernel");
    }

    #[test]
    fn kernel_fault_triggers_panic_path() {
        let out = handle(2, FaultType::PageFault, FaultScope::Kernel, 0x8020_0000, 0xbad);
        match out {
            FaultOutcome::KernelPanic(ev) => {
                assert_eq!(ev.fault_type(), FaultType::KernelInvariantViolation);
                assert_eq!(ev.severity(), Severity::Fatal, "kernel fault is always Fatal");
            }
            other => panic!("expected KernelPanic, got {other:?}"),
        }
    }

    #[test]
    fn kernel_invariant_violation_from_user_scope_is_panic() {
        let mut t = live_thread(1);
        let out =
            handle(3, FaultType::KernelInvariantViolation, FaultScope::User(&mut t), 0, 0);
        assert!(matches!(out, FaultOutcome::KernelPanic(_)));
        assert_eq!(t.state(), ThreadState::Ready, "thread untouched on panic path");
    }

    #[test]
    fn faulting_a_dead_thread_is_an_invariant_violation() {
        let mut t = live_thread(2);
        t.transition(ThreadState::Killed).unwrap();
        let out = handle(4, FaultType::PageFault, FaultScope::User(&mut t), 0, 0);
        assert!(matches!(out, FaultOutcome::KernelPanic(_)));
    }

    #[test]
    fn containment_preserves_other_tasks() {
        // Critical task behavior is preserved: containing a faulty task
        // touches nothing else, and the scheduler still selects the
        // critical task (docs/06, WatchdogTimeout kernel action).
        let critical = live_thread(1);
        let mut faulty = live_thread(2);
        let mut sched = FixedPriorityScheduler::new();
        sched.mark_ready(critical.id(), Priority::MAX).unwrap();
        sched.mark_ready(faulty.id(), Priority::MIN).unwrap();

        faulty.transition(ThreadState::Running).unwrap();
        let out = handle(5, FaultType::WatchdogTimeout, FaultScope::User(&mut faulty), 0, 0);
        assert!(matches!(out, FaultOutcome::Contained(_)));
        sched.mark_not_ready(faulty.id());

        assert_eq!(critical.state(), ThreadState::Ready, "critical task untouched");
        let threads = [&critical, &faulty];
        let selected = sched
            .select_next(|tid| {
                threads.iter().find(|t| t.id() == tid).is_some_and(|t| t.state() == ThreadState::Ready)
            })
            .map(|(tid, _)| tid);
        assert_eq!(selected, Some(critical.id()), "critical task continues");
    }
}
