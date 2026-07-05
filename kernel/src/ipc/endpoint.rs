//! IPC endpoint object (AXIOM-IPC-001).
//!
//! Requirement reference: docs/08_IPC_MODEL.md §3,
//! docs/03_KERNEL_OBJECTS.md §6 (Endpoint).
//!
//! A rendezvous point for synchronous IPC between exactly one sender
//! and one receiver at a time. AXIOM-IPC-001 scope: object, identity,
//! and state model only — the send/receive logic lands with
//! AXIOM-IPC-002.

use crate::thread::ThreadId;

use super::message::Message;

/// Endpoint identifier (kernel-assigned, docs/03 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointId(pub u32);

/// Rendezvous states (docs/03_KERNEL_OBJECTS.md §6). The `Transferring`
/// state of the object model is atomic at this model level: a transfer
/// completes within one kernel operation, so it never rests in a
/// visible intermediate state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointState {
    /// No party waiting.
    Idle,
    /// A sender is blocked with its message, waiting for a receiver.
    SenderWaiting { sender: ThreadId },
    /// A receiver is blocked, waiting for a sender.
    ReceiverWaiting { receiver: ThreadId },
}

/// One IPC endpoint.
#[derive(Debug)]
pub struct Endpoint {
    id: EndpointId,
    state: EndpointState,
    /// The pending message while a sender waits (bounded: exactly one
    /// in-flight message, docs/03 §6 invalid operations).
    pending: Option<Message>,
}

impl Endpoint {
    pub const fn new(id: EndpointId) -> Self {
        Endpoint { id, state: EndpointState::Idle, pending: None }
    }

    pub const fn id(&self) -> EndpointId {
        self.id
    }

    pub const fn state(&self) -> EndpointState {
        self.state
    }

    pub(super) fn set_state(&mut self, state: EndpointState) {
        self.state = state;
    }

    pub(super) fn take_pending(&mut self) -> Option<Message> {
        self.pending.take()
    }

    pub(super) fn put_pending(&mut self, msg: Message) {
        debug_assert!(self.pending.is_none(), "one in-flight message only");
        self.pending = Some(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_start_idle_and_empty() {
        let mut e = Endpoint::new(EndpointId(4));
        assert_eq!(e.id(), EndpointId(4));
        assert_eq!(e.state(), EndpointState::Idle);
        assert!(e.take_pending().is_none());
    }
}
