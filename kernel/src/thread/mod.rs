//! AxiomRT thread model (Phase 5).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md §2 (Thread).
//!
//! Phase 5 scope: thread objects and states only. No context switching,
//! no scheduler integration (Phase 6), no user-mode entry (Phase 7).

pub mod context;
pub mod id;
pub mod state;

pub use context::ThreadContext;
pub use id::ThreadId;
pub use state::{is_legal_transition, ThreadState};

use crate::memory::AddressSpaceId;

/// Failure behavior for invalid thread operations
/// (docs/03_KERNEL_OBJECTS.md §2: invalid state transitions are rejected;
/// at the kernel integration layer they become
/// KernelInvariantViolation checks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: ThreadState,
    pub to: ThreadState,
}

/// Thread object skeleton (docs/03_KERNEL_OBJECTS.md §2).
/// v0.1: one thread per task; created at boot from static descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thread {
    id: ThreadId,
    state: ThreadState,
    /// The task's address space (1:1 in v0.1).
    address_space: AddressSpaceId,
}

impl Thread {
    /// Threads are born Ready (creation is boot-time in v0.1).
    pub const fn new(id: ThreadId, address_space: AddressSpaceId) -> Self {
        Thread { id, state: ThreadState::Ready, address_space }
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }
    pub const fn state(&self) -> ThreadState {
        self.state
    }
    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    /// Apply a state transition. Rejects anything outside the legal
    /// relation in `state::is_legal_transition` and leaves the thread
    /// unchanged on error.
    pub fn transition(&mut self, to: ThreadState) -> Result<(), IllegalTransition> {
        if is_legal_transition(self.state, to) {
            self.state = to;
            Ok(())
        } else {
            Err(IllegalTransition { from: self.state, to })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread() -> Thread {
        Thread::new(ThreadId(1), AddressSpaceId(1))
    }

    #[test]
    fn born_ready() {
        assert_eq!(thread().state(), ThreadState::Ready);
    }

    #[test]
    fn run_yield_cycle() {
        let mut t = thread();
        t.transition(ThreadState::Running).unwrap();
        t.transition(ThreadState::Ready).unwrap();
        t.transition(ThreadState::Running).unwrap();
        t.transition(ThreadState::Blocked).unwrap();
        t.transition(ThreadState::Ready).unwrap();
    }

    #[test]
    fn killed_thread_is_immutable() {
        let mut t = thread();
        t.transition(ThreadState::Killed).unwrap();
        let err = t.transition(ThreadState::Ready).unwrap_err();
        assert_eq!(err, IllegalTransition { from: ThreadState::Killed, to: ThreadState::Ready });
        assert_eq!(t.state(), ThreadState::Killed, "unchanged on error");
    }

    #[test]
    fn faulted_thread_never_scheduled_again() {
        let mut t = thread();
        t.transition(ThreadState::Running).unwrap();
        t.transition(ThreadState::Faulted).unwrap();
        assert!(t.transition(ThreadState::Ready).is_err());
        assert!(t.transition(ThreadState::Running).is_err());
        // Supervisor recovery: suspend then kill is legal.
        t.transition(ThreadState::Suspended).unwrap();
        t.transition(ThreadState::Killed).unwrap();
    }
}
