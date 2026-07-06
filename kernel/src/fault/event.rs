//! FaultEvent model (AXIOM-FAULT-001).
//!
//! Requirement reference: docs/06_FAULT_MODEL.md,
//! docs/03_KERNEL_OBJECTS.md §11 (FaultEvent).
//!
//! Faults are first-class structured events: immutable payload, explicit
//! severity, checked lifecycle (Created → Queued → Delivered →
//! Acknowledged). No recovery policy lives here (AXIOM-FAULT-002/003):
//! this file only says *what happened*, never *what to do*.

use crate::thread::ThreadId;

/// The eight fault types of v0.1 (docs/06_FAULT_MODEL.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultType {
    IllegalSyscall,
    InvalidCapability,
    PageFault,
    IllegalInstruction,
    WatchdogTimeout,
    DeadlineMiss,
    IPCViolation,
    KernelInvariantViolation,
}

/// Severity levels (docs/06_FAULT_MODEL.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Error,
    Critical,
    Fatal,
}

impl FaultType {
    /// Baseline severity per fault type (docs/06; repetition-based
    /// escalation to Critical is supervisor policy, not model data).
    pub const fn severity(self) -> Severity {
        match self {
            FaultType::IllegalSyscall => Severity::Error,
            FaultType::InvalidCapability => Severity::Error,
            FaultType::PageFault => Severity::Error,
            FaultType::IllegalInstruction => Severity::Error,
            FaultType::WatchdogTimeout => Severity::Critical,
            FaultType::DeadlineMiss => Severity::Error,
            FaultType::IPCViolation => Severity::Error,
            FaultType::KernelInvariantViolation => Severity::Fatal,
        }
    }
}

/// Event lifecycle (docs/03_KERNEL_OBJECTS.md §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventState {
    Created,
    Queued,
    Delivered,
    Acknowledged,
}

/// Explicit failure behavior for event lifecycle violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalEventTransition {
    pub from: EventState,
    pub to: EventState,
}

/// One structured fault event. The payload (everything except `state`)
/// is immutable after creation: there are no setters
/// (docs/03 §11 invalid operations: modification after creation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultEvent {
    event_id: u64,
    fault_type: FaultType,
    severity: Severity,
    thread: ThreadId,
    /// Program counter at the fault (0 when not applicable).
    pc: u64,
    /// Fault-specific detail (stval, capability index, syscall number,
    /// ... per the logging fields of docs/06).
    detail: u64,
    state: EventState,
}

impl FaultEvent {
    /// Create an event (kernel-only caller). Severity derives from the
    /// fault type; it is never chosen by the reporter.
    pub const fn new(
        event_id: u64,
        fault_type: FaultType,
        thread: ThreadId,
        pc: u64,
        detail: u64,
    ) -> Self {
        FaultEvent {
            event_id,
            fault_type,
            severity: fault_type.severity(),
            thread,
            pc,
            detail,
            state: EventState::Created,
        }
    }

    pub const fn event_id(&self) -> u64 {
        self.event_id
    }
    pub const fn fault_type(&self) -> FaultType {
        self.fault_type
    }
    pub const fn severity(&self) -> Severity {
        self.severity
    }
    pub const fn thread(&self) -> ThreadId {
        self.thread
    }
    pub const fn pc(&self) -> u64 {
        self.pc
    }
    pub const fn detail(&self) -> u64 {
        self.detail
    }
    pub const fn state(&self) -> EventState {
        self.state
    }

    /// Advance the lifecycle. Only the forward chain is legal.
    pub fn advance(&mut self, to: EventState) -> Result<(), IllegalEventTransition> {
        use EventState::*;
        let legal = matches!(
            (self.state, to),
            (Created, Queued) | (Queued, Delivered) | (Delivered, Acknowledged)
        );
        if legal {
            self.state = to;
            Ok(())
        } else {
            Err(IllegalEventTransition {
                from: self.state,
                to,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> FaultEvent {
        FaultEvent::new(1, FaultType::PageFault, ThreadId(3), 0x8020_0abc, 0xdead)
    }

    #[test]
    fn severity_is_derived_not_chosen() {
        assert_eq!(event().severity(), Severity::Error);
        assert_eq!(
            FaultEvent::new(2, FaultType::KernelInvariantViolation, ThreadId(0), 0, 0).severity(),
            Severity::Fatal
        );
        assert_eq!(
            FaultEvent::new(3, FaultType::WatchdogTimeout, ThreadId(1), 0, 0).severity(),
            Severity::Critical
        );
    }

    #[test]
    fn lifecycle_forward_chain_only() {
        let mut e = event();
        assert_eq!(e.state(), EventState::Created);
        e.advance(EventState::Queued).unwrap();
        e.advance(EventState::Delivered).unwrap();
        e.advance(EventState::Acknowledged).unwrap();
    }

    #[test]
    fn skipping_and_rewinding_rejected() {
        let mut e = event();
        assert!(
            e.advance(EventState::Delivered).is_err(),
            "cannot skip Queued"
        );
        assert!(e.advance(EventState::Acknowledged).is_err());
        e.advance(EventState::Queued).unwrap();
        assert!(e.advance(EventState::Created).is_err(), "no rewind");
        e.advance(EventState::Delivered).unwrap();
        e.advance(EventState::Acknowledged).unwrap();
        assert!(e.advance(EventState::Acknowledged).is_err(), "terminal");
    }

    #[test]
    fn payload_is_immutable_by_construction() {
        // No setters exist; this test pins the accessor values.
        let e = event();
        assert_eq!(e.event_id(), 1);
        assert_eq!(e.fault_type(), FaultType::PageFault);
        assert_eq!(e.thread(), ThreadId(3));
        assert_eq!(e.pc(), 0x8020_0abc);
        assert_eq!(e.detail(), 0xdead);
    }
}
