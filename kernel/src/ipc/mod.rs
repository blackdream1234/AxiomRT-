//! AxiomRT synchronous IPC (Phase 8).
//!
//! Requirement reference: docs/08_IPC_MODEL.md,
//! docs/02_KERNEL_BLUEPRINT.md §9 (IPC principle).
//!
//! Synchronous, bounded, copy-based. No shared memory. No buffering
//! beyond the single in-flight rendezvous message. AXIOM-IPC-001 scope:
//! object model (Endpoint, Message, states). Send/receive logic:
//! AXIOM-IPC-002. Capability integration: Phase 9 (AXIOM-CAP-003) —
//! nothing here bypasses capability checks; the syscall layer will
//! perform them before reaching this model.

pub mod endpoint;
pub mod message;

pub use endpoint::{Endpoint, EndpointId, EndpointState};
pub use message::{Message, MessageError, MessageHeader, MSG_MAX_BYTES};

use crate::thread::ThreadId;

/// Explicit failure behavior for rendezvous operations
/// (docs/08_IPC_MODEL.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// Another sender (or receiver) already waits on this endpoint:
    /// bounded, no queues in v0.1.
    Busy,
    /// The caller is already the parked party on this endpoint.
    AlreadyWaiting,
}

/// Outcome of a send operation (AXIOM-IPC-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// No receiver present: the sender must block
    /// (thread → Blocked, docs/03_KERNEL_OBJECTS.md §2).
    Blocked,
    /// A receiver was waiting: message delivered, both continue.
    Delivered { to: ThreadId, msg: Message },
}

/// Outcome of a receive operation (AXIOM-IPC-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvOutcome {
    /// No sender present: the receiver must block.
    Blocked,
    /// A sender was waiting: message received, unblock that sender.
    Received { msg: Message, unblock: ThreadId },
}

/// Outcome of cancelling a party (kill path,
/// docs/03_KERNEL_OBJECTS.md §6 failure behavior).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The thread was not parked here.
    NotWaiting,
    /// The parked party was removed; endpoint is Idle again. The
    /// in-flight message (if any) is dropped undelivered — the
    /// receiver observes nothing (docs/04, no partial messages).
    Cancelled,
}

/// Synchronous rendezvous: send `msg` from `sender` on `ep`.
///
/// Deterministic: the outcome is a pure function of the endpoint state
/// (docs/08_IPC_MODEL.md §4). Capability checks happen at the syscall
/// layer (Phase 9) before this function is reached.
pub fn send(ep: &mut Endpoint, sender: ThreadId, msg: Message) -> Result<SendOutcome, IpcError> {
    match ep.state() {
        EndpointState::Idle => {
            ep.put_pending(msg);
            ep.set_state(EndpointState::SenderWaiting { sender });
            Ok(SendOutcome::Blocked)
        }
        EndpointState::ReceiverWaiting { receiver } => {
            // Atomic transfer: bounded copy to the receiver, endpoint
            // returns to Idle, both parties continue.
            ep.set_state(EndpointState::Idle);
            Ok(SendOutcome::Delivered { to: receiver, msg })
        }
        EndpointState::SenderWaiting { sender: s } => {
            if s == sender {
                Err(IpcError::AlreadyWaiting)
            } else {
                Err(IpcError::Busy)
            }
        }
    }
}

/// Synchronous rendezvous: receive on `ep` as `receiver`.
pub fn recv(ep: &mut Endpoint, receiver: ThreadId) -> Result<RecvOutcome, IpcError> {
    match ep.state() {
        EndpointState::Idle => {
            ep.set_state(EndpointState::ReceiverWaiting { receiver });
            Ok(RecvOutcome::Blocked)
        }
        EndpointState::SenderWaiting { sender } => {
            let msg = ep
                .take_pending()
                .expect("kernel invariant: SenderWaiting implies a pending message");
            ep.set_state(EndpointState::Idle);
            Ok(RecvOutcome::Received {
                msg,
                unblock: sender,
            })
        }
        EndpointState::ReceiverWaiting { receiver: r } => {
            if r == receiver {
                Err(IpcError::AlreadyWaiting)
            } else {
                Err(IpcError::Busy)
            }
        }
    }
}

// ---------------------------------------------------------------------
// Capability-checked entry points (AXIOM-CAP-003).
//
// The only lawful path from a syscall to the rendezvous model. The
// capability must (a) pass the table lookup with the required right and
// (b) reference *this* endpoint — holding Send on endpoint A grants
// nothing on endpoint B.

use crate::caps::{CapError, CapTable, ObjectType, Rights};

/// Failure behavior of checked IPC (docs/06_CAPABILITY_MODEL.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcCapError {
    /// Capability lookup failed → InvalidCapability fault path
    /// (CAP_DENIED / IPC_DENIED event, docs/06_FAULT_MODEL.md).
    Cap(CapError),
    /// The capability is valid but references a different endpoint.
    WrongEndpoint,
    /// Rendezvous-level error (Busy / AlreadyWaiting).
    Ipc(IpcError),
}

/// sys_send path: requires an Endpoint capability with the Send right
/// that references `ep`. The endpoint is never touched on failure.
pub fn send_checked(
    table: &CapTable,
    cap_index: usize,
    ep: &mut Endpoint,
    sender: ThreadId,
    msg: Message,
) -> Result<SendOutcome, IpcCapError> {
    let obj = table
        .lookup(cap_index, ObjectType::Endpoint, Rights::SEND)
        .map_err(IpcCapError::Cap)?;
    if obj.object_id != ep.id().0 {
        return Err(IpcCapError::WrongEndpoint);
    }
    send(ep, sender, msg).map_err(IpcCapError::Ipc)
}

/// sys_recv path: requires an Endpoint capability with the Receive
/// right that references `ep`.
pub fn recv_checked(
    table: &CapTable,
    cap_index: usize,
    ep: &mut Endpoint,
    receiver: ThreadId,
) -> Result<RecvOutcome, IpcCapError> {
    let obj = table
        .lookup(cap_index, ObjectType::Endpoint, Rights::RECEIVE)
        .map_err(IpcCapError::Cap)?;
    if obj.object_id != ep.id().0 {
        return Err(IpcCapError::WrongEndpoint);
    }
    recv(ep, receiver).map_err(IpcCapError::Ipc)
}

/// Cancel a parked party (thread killed while blocked). The kernel
/// unblocks the peer with ERR_PEER_KILLED at the syscall layer.
pub fn cancel(ep: &mut Endpoint, tid: ThreadId) -> CancelOutcome {
    match ep.state() {
        EndpointState::SenderWaiting { sender } if sender == tid => {
            let _ = ep.take_pending(); // dropped undelivered
            ep.set_state(EndpointState::Idle);
            CancelOutcome::Cancelled
        }
        EndpointState::ReceiverWaiting { receiver } if receiver == tid => {
            ep.set_state(EndpointState::Idle);
            CancelOutcome::Cancelled
        }
        _ => CancelOutcome::NotWaiting,
    }
}
