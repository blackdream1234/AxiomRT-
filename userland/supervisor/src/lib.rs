//! AxiomRT supervisor service (AXIOM-FAULT-003).
//!
//! Requirement reference: docs/06_FAULT_MODEL.md, Project Description
//! §17 (supervisor model).
//!
//! The supervisor is a *trusted user-space service*: trusted for
//! recovery **policy**, not for violating isolation. It receives fault
//! events exclusively through capability-checked IPC on the fault
//! channel (Receive right) and answers with an explicit
//! `RecoveryDecision`. It has no privileged instructions, no raw
//! object access, and no path around the capability system.

#![cfg_attr(not(test), no_std)]

use kernel::caps::CapTable;
use kernel::fault::{decode, DecodeError, FaultReport, FaultType, RecoveryDecision, Severity};
use kernel::ipc::{recv_checked, Endpoint, IpcCapError, RecvOutcome};
use kernel::thread::ThreadId;

/// Recovery policy (v0.1): the documented defaults of
/// docs/06_FAULT_MODEL.md, with severity escalation — Critical faults
/// of a restartable kind are restarted rather than killed so that
/// supervised services come back, and repeated-abuse escalation stays
/// with the caller (event history is a v0.2+ concern).
pub fn decide(report: &FaultReport) -> RecoveryDecision {
    match (report.fault_type, report.severity) {
        (FaultType::WatchdogTimeout, _) => RecoveryDecision::Restart,
        (FaultType::DeadlineMiss, Severity::Critical) => RecoveryDecision::Restart,
        (FaultType::DeadlineMiss, _) => RecoveryDecision::Escalate,
        (FaultType::InvalidCapability, Severity::Critical) => RecoveryDecision::Quarantine,
        (FaultType::InvalidCapability, _) => RecoveryDecision::Escalate,
        (FaultType::IllegalSyscall, _)
        | (FaultType::PageFault, _)
        | (FaultType::IllegalInstruction, _)
        | (FaultType::IPCViolation, _) => RecoveryDecision::Kill,
        // Never reaches the supervisor (the kernel has halted); decided
        // here only to keep the policy total.
        (FaultType::KernelInvariantViolation, _) => RecoveryDecision::Escalate,
    }
}

/// Explicit failure behavior of one poll step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollError {
    /// Capability check failed — the supervisor holds no (sufficient)
    /// Receive capability for the fault channel. There is no fallback
    /// path: no capability, no events.
    Denied,
    /// No fault event is pending (the supervisor would block).
    NothingPending,
    /// The message on the channel was not a valid fault report.
    BadReport(DecodeError),
}

/// One supervisor poll: receive a fault event over the checked IPC
/// path, decode it, and produce the explicit recovery decision that
/// the kernel applies via sys_fault_ack (docs/04_SYSCALL_MODEL.md).
pub fn poll_fault_channel(
    table: &CapTable,
    cap_index: usize,
    channel: &mut Endpoint,
    supervisor_tid: ThreadId,
) -> Result<(FaultReport, RecoveryDecision), PollError> {
    match recv_checked(table, cap_index, channel, supervisor_tid) {
        Err(IpcCapError::Cap(_)) | Err(IpcCapError::WrongEndpoint) => Err(PollError::Denied),
        Err(IpcCapError::Ipc(_)) => Err(PollError::NothingPending),
        Ok(RecvOutcome::Blocked) => Err(PollError::NothingPending),
        Ok(RecvOutcome::Received { msg, .. }) => {
            let report = decode(&msg).map_err(PollError::BadReport)?;
            Ok((report, decide(&report)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::caps::{Capability, ObjectRef, ObjectType, Rights};
    use kernel::fault::{notify_supervisor, FaultEvent, NotifyOutcome};
    use kernel::ipc::EndpointId;

    const CHANNEL_ID: u32 = 9;
    const SUPERVISOR: ThreadId = ThreadId(10);

    fn channel() -> Endpoint {
        Endpoint::new(EndpointId(CHANNEL_ID))
    }

    fn supervisor_table() -> CapTable {
        let mut t = CapTable::new();
        t.insert(
            0,
            Capability::new(
                ObjectRef { object_type: ObjectType::Endpoint, object_id: CHANNEL_ID },
                Rights::RECEIVE.union(Rights::CONTROL),
            ),
        )
        .unwrap();
        t
    }

    #[test]
    fn fault_event_reaches_supervisor_and_yields_explicit_decision() {
        let mut ch = channel();
        let mut ev = FaultEvent::new(7, FaultType::PageFault, ThreadId(3), 0xabc, 0xbad);
        assert_eq!(notify_supervisor(&mut ch, &mut ev), NotifyOutcome::Queued);

        let table = supervisor_table();
        let (report, decision) =
            poll_fault_channel(&table, 0, &mut ch, SUPERVISOR).expect("event must arrive");
        assert_eq!(report.event_id, 7);
        assert_eq!(report.thread, ThreadId(3));
        assert_eq!(decision, RecoveryDecision::Kill, "explicit decision, per policy");
    }

    #[test]
    fn no_capability_means_no_fault_events() {
        let mut ch = channel();
        let mut ev = FaultEvent::new(8, FaultType::IllegalSyscall, ThreadId(4), 0, 0);
        notify_supervisor(&mut ch, &mut ev);

        let empty = CapTable::new();
        assert_eq!(
            poll_fault_channel(&empty, 0, &mut ch, SUPERVISOR),
            Err(PollError::Denied),
            "supervisor cannot bypass capabilities"
        );

        // A capability for a *different* endpoint does not help either.
        let mut wrong = CapTable::new();
        wrong
            .insert(
                0,
                Capability::new(
                    ObjectRef { object_type: ObjectType::Endpoint, object_id: 999 },
                    Rights::RECEIVE,
                ),
            )
            .unwrap();
        assert_eq!(poll_fault_channel(&wrong, 0, &mut ch, SUPERVISOR), Err(PollError::Denied));
    }

    #[test]
    fn empty_channel_reports_nothing_pending() {
        let mut ch = channel();
        let table = supervisor_table();
        assert_eq!(
            poll_fault_channel(&table, 0, &mut ch, SUPERVISOR),
            Err(PollError::NothingPending)
        );
    }

    #[test]
    fn policy_matches_documented_defaults() {
        let mk = |ft: FaultType| FaultReport {
            event_id: 1,
            fault_type: ft,
            severity: ft.severity(),
            thread: ThreadId(2),
            pc: 0,
            detail: 0,
        };
        assert_eq!(decide(&mk(FaultType::WatchdogTimeout)), RecoveryDecision::Restart);
        assert_eq!(decide(&mk(FaultType::PageFault)), RecoveryDecision::Kill);
        assert_eq!(decide(&mk(FaultType::IllegalInstruction)), RecoveryDecision::Kill);
        assert_eq!(decide(&mk(FaultType::IllegalSyscall)), RecoveryDecision::Kill);
        assert_eq!(decide(&mk(FaultType::IPCViolation)), RecoveryDecision::Kill);
        assert_eq!(decide(&mk(FaultType::InvalidCapability)), RecoveryDecision::Escalate);
        assert_eq!(decide(&mk(FaultType::DeadlineMiss)), RecoveryDecision::Escalate);
    }
}
