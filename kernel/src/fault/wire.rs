//! Supervisor notification path (AXIOM-FAULT-003).
//!
//! Requirement reference: docs/06_FAULT_MODEL.md (supervisor
//! notification), docs/08_IPC_MODEL.md, Project Description §17.
//!
//! Fault events reach the supervisor through the same synchronous,
//! bounded, copy-based IPC as any other message — over a dedicated
//! fault channel endpoint. The kernel is the sender (it is the TCB and
//! mints the channel at boot); the supervisor receives with a Receive
//! capability through the ordinary checked path. **There is no bypass:
//! the supervisor cannot obtain fault events, or anything else,
//! without the corresponding capability.**

use crate::ipc::{self, Endpoint, Message, SendOutcome};
use crate::thread::ThreadId;

use super::event::{EventState, FaultEvent, FaultType, Severity};

/// Sender identity the kernel uses on the fault channel. ThreadId 0 is
/// reserved for the kernel itself (never a schedulable task in v0.1).
pub const KERNEL_SENDER: ThreadId = ThreadId(0);

/// Wire size of one encoded fault event (see `encode`).
pub const FAULT_WIRE_BYTES: usize = 30;

/// Explicit recovery decisions the supervisor can take
/// (docs/06_FAULT_MODEL.md). KernelPanic is deliberately NOT here: it
/// is never selectable by the supervisor for user faults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    Kill,
    Restart,
    Suspend,
    Quarantine,
    Escalate,
}

/// Default policy per fault type when the supervisor is unavailable
/// (the bold defaults of docs/06_FAULT_MODEL.md). Total over user
/// fault types; KernelInvariantViolation has no recovery decision.
pub const fn default_decision(fault_type: FaultType) -> Option<RecoveryDecision> {
    match fault_type {
        FaultType::IllegalSyscall => Some(RecoveryDecision::Kill),
        FaultType::InvalidCapability => Some(RecoveryDecision::Escalate),
        FaultType::PageFault => Some(RecoveryDecision::Kill),
        FaultType::IllegalInstruction => Some(RecoveryDecision::Kill),
        FaultType::WatchdogTimeout => Some(RecoveryDecision::Restart),
        FaultType::DeadlineMiss => Some(RecoveryDecision::Escalate),
        FaultType::IPCViolation => Some(RecoveryDecision::Kill),
        FaultType::KernelInvariantViolation => None,
    }
}

/// Is `decision` a legal supervisor response to `fault_type`?
/// (docs/06: user faults admit the five decisions; a kernel invariant
/// violation admits none — the system has already halted.)
pub const fn is_valid_recovery(fault_type: FaultType, _decision: RecoveryDecision) -> bool {
    !matches!(fault_type, FaultType::KernelInvariantViolation)
}

/// Outcome of a notification attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyOutcome {
    /// Supervisor was blocked receiving: event handed over immediately
    /// (event state → Delivered).
    Delivered,
    /// No receiver present: the event is parked in the channel
    /// (endpoint SenderWaiting) until the supervisor receives it
    /// (event state → Queued).
    Queued,
    /// Channel already holds an undelivered event (bounded, no queue in
    /// v0.1): caller applies the default policy for this fault type and
    /// records the delivery failure (docs/06 default-policy rule).
    ChannelFull,
}

fn fault_type_to_u8(t: FaultType) -> u8 {
    match t {
        FaultType::IllegalSyscall => 0,
        FaultType::InvalidCapability => 1,
        FaultType::PageFault => 2,
        FaultType::IllegalInstruction => 3,
        FaultType::WatchdogTimeout => 4,
        FaultType::DeadlineMiss => 5,
        FaultType::IPCViolation => 6,
        FaultType::KernelInvariantViolation => 7,
    }
}

fn fault_type_from_u8(v: u8) -> Option<FaultType> {
    Some(match v {
        0 => FaultType::IllegalSyscall,
        1 => FaultType::InvalidCapability,
        2 => FaultType::PageFault,
        3 => FaultType::IllegalInstruction,
        4 => FaultType::WatchdogTimeout,
        5 => FaultType::DeadlineMiss,
        6 => FaultType::IPCViolation,
        7 => FaultType::KernelInvariantViolation,
        _ => return None,
    })
}

/// Encode a fault event for the wire (fixed 30-byte layout, LE):
/// `[0..8) event_id | [8) fault_type | [9) severity |
///  [10..14) thread | [14..22) pc | [22..30) detail`.
pub fn encode(event: &FaultEvent) -> Message {
    let mut buf = [0u8; FAULT_WIRE_BYTES];
    buf[0..8].copy_from_slice(&event.event_id().to_le_bytes());
    buf[8] = fault_type_to_u8(event.fault_type());
    buf[9] = event.severity() as u8;
    buf[10..14].copy_from_slice(&event.thread().as_u32().to_le_bytes());
    buf[14..22].copy_from_slice(&event.pc().to_le_bytes());
    buf[22..30].copy_from_slice(&event.detail().to_le_bytes());
    Message::new(KERNEL_SENDER, &buf)
        .expect("kernel invariant: FAULT_WIRE_BYTES <= MSG_MAX_BYTES")
}

/// A decoded fault report on the supervisor side (a *report* about the
/// kernel-owned event, not the event object itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultReport {
    pub event_id: u64,
    pub fault_type: FaultType,
    pub severity: Severity,
    pub thread: ThreadId,
    pub pc: u64,
    pub detail: u64,
}

/// Explicit decode failure behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    WrongLength,
    UnknownFaultType,
    /// Severity byte inconsistent with the fault type (severity is
    /// derived, never free — a mismatch means corruption or forgery).
    SeverityMismatch,
    /// Fault reports are only ever sent by the kernel.
    ForgedSender,
}

/// Decode and validate a fault report message.
pub fn decode(msg: &Message) -> Result<FaultReport, DecodeError> {
    if msg.header().sender != KERNEL_SENDER {
        return Err(DecodeError::ForgedSender);
    }
    let data = msg.data();
    if data.len() != FAULT_WIRE_BYTES {
        return Err(DecodeError::WrongLength);
    }
    let fault_type = fault_type_from_u8(data[8]).ok_or(DecodeError::UnknownFaultType)?;
    if data[9] != fault_type.severity() as u8 {
        return Err(DecodeError::SeverityMismatch);
    }
    Ok(FaultReport {
        event_id: u64::from_le_bytes(data[0..8].try_into().expect("length checked")),
        fault_type,
        severity: fault_type.severity(),
        thread: ThreadId(u32::from_le_bytes(data[10..14].try_into().expect("length checked"))),
        pc: u64::from_le_bytes(data[14..22].try_into().expect("length checked")),
        detail: u64::from_le_bytes(data[22..30].try_into().expect("length checked")),
    })
}

/// Kernel-side notification: put the event on the fault channel.
/// The kernel never blocks: "Blocked" from the rendezvous model means
/// the message is parked in the channel (→ Queued).
pub fn notify_supervisor(
    channel: &mut Endpoint,
    event: &mut FaultEvent,
) -> NotifyOutcome {
    let msg = encode(event);
    match ipc::send(channel, KERNEL_SENDER, msg) {
        Ok(SendOutcome::Delivered { .. }) => {
            event.advance(EventState::Queued).expect("Created -> Queued");
            event.advance(EventState::Delivered).expect("Queued -> Delivered");
            NotifyOutcome::Delivered
        }
        Ok(SendOutcome::Blocked) => {
            event.advance(EventState::Queued).expect("Created -> Queued");
            NotifyOutcome::Queued
        }
        Err(_) => NotifyOutcome::ChannelFull,
    }
}

/// Explicit failure behavior for acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckError {
    /// Decision not in the legal set for this fault type.
    InvalidDecision,
    /// Event is not in the Delivered state (double ack, or never
    /// delivered) — maps to ERR_NO_PENDING_FAULT (docs/04).
    NotPending,
}

/// Close the loop: the supervisor's explicit decision is recorded and
/// the event becomes Acknowledged (sys_fault_ack semantics, docs/04).
pub fn acknowledge(
    event: &mut FaultEvent,
    decision: RecoveryDecision,
) -> Result<RecoveryDecision, AckError> {
    if !is_valid_recovery(event.fault_type(), decision) {
        return Err(AckError::InvalidDecision);
    }
    if event.state() != EventState::Delivered {
        return Err(AckError::NotPending);
    }
    event.advance(EventState::Acknowledged).expect("Delivered -> Acknowledged");
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{Capability, CapTable, ObjectRef, ObjectType, Rights};
    use crate::ipc::{recv_checked, EndpointId, IpcCapError, RecvOutcome};

    fn event() -> FaultEvent {
        FaultEvent::new(42, FaultType::PageFault, ThreadId(3), 0x8020_0abc, 0xbad)
    }

    fn fault_channel() -> Endpoint {
        Endpoint::new(EndpointId(9))
    }

    fn supervisor_table() -> CapTable {
        let mut t = CapTable::new();
        t.insert(
            0,
            Capability::new(
                ObjectRef { object_type: ObjectType::Endpoint, object_id: 9 },
                Rights::RECEIVE.union(Rights::CONTROL),
            ),
        )
        .unwrap();
        t
    }

    #[test]
    fn wire_round_trip() {
        let ev = event();
        let msg = encode(&ev);
        let report = decode(&msg).unwrap();
        assert_eq!(report.event_id, 42);
        assert_eq!(report.fault_type, FaultType::PageFault);
        assert_eq!(report.severity, Severity::Error);
        assert_eq!(report.thread, ThreadId(3));
        assert_eq!(report.pc, 0x8020_0abc);
        assert_eq!(report.detail, 0xbad);
    }

    #[test]
    fn decode_rejects_forged_and_corrupt_reports() {
        // Forged sender.
        let forged = Message::new(ThreadId(5), &[0u8; FAULT_WIRE_BYTES]).unwrap();
        assert_eq!(decode(&forged), Err(DecodeError::ForgedSender));
        // Wrong length.
        let short = Message::new(KERNEL_SENDER, &[0u8; 4]).unwrap();
        assert_eq!(decode(&short), Err(DecodeError::WrongLength));
        // Unknown fault type.
        let mut buf = [0u8; FAULT_WIRE_BYTES];
        buf[8] = 200;
        let bad_type = Message::new(KERNEL_SENDER, &buf).unwrap();
        assert_eq!(decode(&bad_type), Err(DecodeError::UnknownFaultType));
        // Severity forgery: PageFault with Fatal byte.
        let mut buf = [0u8; FAULT_WIRE_BYTES];
        buf[8] = 2; // PageFault
        buf[9] = Severity::Fatal as u8;
        let forged_sev = Message::new(KERNEL_SENDER, &buf).unwrap();
        assert_eq!(decode(&forged_sev), Err(DecodeError::SeverityMismatch));
    }

    #[test]
    fn fault_event_reaches_supervisor_through_checked_ipc() {
        let mut channel = fault_channel();
        let mut ev = event();

        // Kernel notifies first: event parks in the channel.
        assert_eq!(notify_supervisor(&mut channel, &mut ev), NotifyOutcome::Queued);
        assert_eq!(ev.state(), EventState::Queued);

        // Supervisor receives through the capability-checked path.
        let table = supervisor_table();
        match recv_checked(&table, 0, &mut channel, ThreadId(10)).unwrap() {
            RecvOutcome::Received { msg, unblock } => {
                assert_eq!(unblock, KERNEL_SENDER);
                let report = decode(&msg).unwrap();
                assert_eq!(report.event_id, 42);
                assert_eq!(report.thread, ThreadId(3));
            }
            other => panic!("expected Received, got {other:?}"),
        }
        // Kernel marks delivery once the supervisor picked it up.
        ev.advance(EventState::Delivered).unwrap();

        // Explicit recovery decision closes the loop.
        assert_eq!(acknowledge(&mut ev, RecoveryDecision::Kill), Ok(RecoveryDecision::Kill));
        assert_eq!(ev.state(), EventState::Acknowledged);
    }

    #[test]
    fn supervisor_cannot_bypass_capabilities() {
        let mut channel = fault_channel();
        let mut ev = event();
        notify_supervisor(&mut channel, &mut ev);

        // No capability: no fault events.
        let empty = CapTable::new();
        assert!(matches!(
            recv_checked(&empty, 0, &mut channel, ThreadId(10)),
            Err(IpcCapError::Cap(_))
        ));

        // Send-right-only capability on the channel: still no receive.
        let mut wrong = CapTable::new();
        wrong
            .insert(
                0,
                Capability::new(
                    ObjectRef { object_type: ObjectType::Endpoint, object_id: 9 },
                    Rights::SEND,
                ),
            )
            .unwrap();
        assert!(matches!(
            recv_checked(&wrong, 0, &mut channel, ThreadId(10)),
            Err(IpcCapError::Cap(_))
        ));
    }

    #[test]
    fn bounded_channel_falls_back_to_default_policy() {
        let mut channel = fault_channel();
        let mut first = event();
        let mut second = FaultEvent::new(43, FaultType::IllegalSyscall, ThreadId(4), 0, 7);
        assert_eq!(notify_supervisor(&mut channel, &mut first), NotifyOutcome::Queued);
        assert_eq!(
            notify_supervisor(&mut channel, &mut second),
            NotifyOutcome::ChannelFull,
            "second undelivered event cannot queue (bounded)"
        );
        // Caller applies the documented default for the dropped event.
        assert_eq!(default_decision(second.fault_type()), Some(RecoveryDecision::Kill));
    }

    #[test]
    fn recovery_decisions_are_validated() {
        let mut ev = event();
        ev.advance(EventState::Queued).unwrap();
        ev.advance(EventState::Delivered).unwrap();
        // Double-ack rejected.
        acknowledge(&mut ev, RecoveryDecision::Restart).unwrap();
        assert_eq!(acknowledge(&mut ev, RecoveryDecision::Kill), Err(AckError::NotPending));

        // KernelInvariantViolation admits no supervisor decision.
        let mut fatal = FaultEvent::new(9, FaultType::KernelInvariantViolation, ThreadId(0), 0, 0);
        assert_eq!(
            acknowledge(&mut fatal, RecoveryDecision::Kill),
            Err(AckError::InvalidDecision)
        );
        assert_eq!(default_decision(FaultType::KernelInvariantViolation), None);
    }
}
