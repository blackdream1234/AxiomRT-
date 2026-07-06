//! IPC message object (AXIOM-IPC-001).
//!
//! Requirement reference: docs/08_IPC_MODEL.md §2,
//! docs/03_KERNEL_OBJECTS.md §7 (Message).
//!
//! Bounded, copy-based: a message is a fixed-capacity byte buffer plus a
//! kernel-written header. There is no pointer transfer and no shared
//! memory — constructing a Message *copies* the payload, and delivery
//! copies it again into the receiver (no aliasing at any point).

use crate::thread::ThreadId;

/// Fixed maximum message size in bytes (docs/08_IPC_MODEL.md §2).
pub const MSG_MAX_BYTES: usize = 64;

/// Explicit failure behavior for message construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageError {
    /// Payload exceeds MSG_MAX_BYTES (ERR_MSG_TOO_LARGE at the syscall
    /// layer, docs/04_SYSCALL_MODEL.md).
    TooLarge,
}

/// Kernel-written message header. The sender identity is set by the
/// kernel at send time and cannot be forged by user code
/// (docs/04_SYSCALL_MODEL.md, sys_recv security rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub sender: ThreadId,
    pub len: usize,
}

/// One bounded, copied message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message {
    header: MessageHeader,
    payload: [u8; MSG_MAX_BYTES],
}

impl Message {
    /// Build a message by copying `data`. Rejects oversized payloads
    /// before any copy happens (validation-before-use,
    /// docs/04_SYSCALL_MODEL.md general rules).
    pub fn new(sender: ThreadId, data: &[u8]) -> Result<Self, MessageError> {
        if data.len() > MSG_MAX_BYTES {
            return Err(MessageError::TooLarge);
        }
        let mut payload = [0u8; MSG_MAX_BYTES];
        payload[..data.len()].copy_from_slice(data);
        Ok(Message {
            header: MessageHeader {
                sender,
                len: data.len(),
            },
            payload,
        })
    }

    pub const fn header(&self) -> MessageHeader {
        self.header
    }

    /// The payload bytes (exactly `header.len` of them).
    pub fn data(&self) -> &[u8] {
        &self.payload[..self.header.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_construction() {
        let m = Message::new(ThreadId(1), &[1, 2, 3]).unwrap();
        assert_eq!(m.header().sender, ThreadId(1));
        assert_eq!(m.data(), &[1, 2, 3]);
    }

    #[test]
    fn oversized_rejected_before_copy() {
        let big = [0u8; MSG_MAX_BYTES + 1];
        assert_eq!(Message::new(ThreadId(1), &big), Err(MessageError::TooLarge));
    }

    #[test]
    fn max_size_accepted() {
        let max = [7u8; MSG_MAX_BYTES];
        let m = Message::new(ThreadId(2), &max).unwrap();
        assert_eq!(m.data().len(), MSG_MAX_BYTES);
    }

    #[test]
    fn message_is_a_copy_not_a_view() {
        let mut src = [9u8; 4];
        let m = Message::new(ThreadId(3), &src).unwrap();
        src[0] = 0; // mutating the source after send must not matter
        assert_eq!(src[0], 0, "source buffer was really mutated");
        assert_eq!(
            m.data(),
            &[9, 9, 9, 9],
            "no shared memory: payload was copied"
        );
    }
}
