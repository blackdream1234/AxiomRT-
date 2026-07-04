//! Thread states and the legal transition relation (AXIOM-THREAD-001).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md §2 (Thread),
//! docs/06_FAULT_MODEL.md (fault handling invariants).
//!
//! The transition relation is total and explicit: everything not listed
//! in `is_legal_transition` is an invalid operation. Key rules:
//!   * Killed is terminal — no resurrection.
//!   * Faulted never returns to execution: recovery is Kill or Suspend/
//!     Quarantine; Restart creates a *fresh* thread
//!     (docs/06 invariant 3).
//!   * Only Ready threads become Running (scheduler rule).

/// Thread lifecycle states (docs/03_KERNEL_OBJECTS.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Runnable, waiting for the scheduler.
    Ready,
    /// Currently executing on the hart.
    Running,
    /// Blocked in a synchronous IPC rendezvous.
    Blocked,
    /// Stopped by a fault; awaiting supervisor decision (terminal for
    /// execution: never scheduled again as-is).
    Faulted,
    /// Terminated permanently (exit or Kill recovery). Terminal.
    Killed,
    /// Frozen by the supervisor (Suspend or Quarantine recovery); may be
    /// resumed unless quarantined by policy.
    Suspended,
}

/// The complete legal transition relation.
pub fn is_legal_transition(from: ThreadState, to: ThreadState) -> bool {
    use ThreadState::*;
    matches!(
        (from, to),
        // Scheduler selection and descheduling.
        (Ready, Running)
        | (Running, Ready)
        // Synchronous IPC rendezvous.
        | (Running, Blocked)
        | (Blocked, Ready)
        // Fault containment: a live thread can fault (running fault,
        // watchdog on ready/blocked threads).
        | (Ready, Faulted)
        | (Running, Faulted)
        | (Blocked, Faulted)
        // Termination: exit or supervisor Kill of any live or faulted
        // or suspended thread.
        | (Ready, Killed)
        | (Running, Killed)
        | (Blocked, Killed)
        | (Faulted, Killed)
        | (Suspended, Killed)
        // Supervisor freeze/unfreeze.
        | (Ready, Suspended)
        | (Running, Suspended)
        | (Blocked, Suspended)
        | (Faulted, Suspended)
        | (Suspended, Ready)
    )
}

#[cfg(test)]
mod tests {
    use super::ThreadState::*;
    use super::*;

    const ALL: [ThreadState; 6] = [Ready, Running, Blocked, Faulted, Killed, Suspended];

    #[test]
    fn killed_is_terminal() {
        for to in ALL {
            assert!(!is_legal_transition(Killed, to), "Killed -> {to:?} must be illegal");
        }
    }

    #[test]
    fn faulted_never_returns_to_execution() {
        assert!(!is_legal_transition(Faulted, Ready));
        assert!(!is_legal_transition(Faulted, Running));
        assert!(!is_legal_transition(Faulted, Blocked));
        // Recovery paths that are legal:
        assert!(is_legal_transition(Faulted, Killed));
        assert!(is_legal_transition(Faulted, Suspended));
    }

    #[test]
    fn only_ready_becomes_running() {
        for from in ALL {
            let legal = is_legal_transition(from, Running);
            assert_eq!(legal, from == Ready, "{from:?} -> Running");
        }
    }

    #[test]
    fn blocked_unblocks_to_ready_not_running() {
        assert!(is_legal_transition(Blocked, Ready));
        assert!(!is_legal_transition(Blocked, Running));
    }

    #[test]
    fn no_self_transitions() {
        for s in ALL {
            assert!(!is_legal_transition(s, s), "{s:?} -> {s:?} must be illegal");
        }
    }
}
