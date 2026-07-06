//! Capability lookup table (AXIOM-CAP-002).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md §4.
//!
//! One table per task, fixed capacity, no heap. This is the single
//! enforcement point of the security model: every syscall resolves its
//! object references here, in a fixed check order, before any object
//! is touched. Error behavior is explicit and total.

use super::capability::{Capability, ObjectRef, ObjectType};
use super::rights::Rights;

/// Capability slots per task (static, v0.1).
pub const CAP_TABLE_SLOTS: usize = 32;

/// Explicit failure behavior (docs/06_CAPABILITY_MODEL.md §4; mapped
/// to syscall result codes in docs/04_SYSCALL_MODEL.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapError {
    /// Index outside the table (→ ERR_INVALID_CAP).
    InvalidIndex,
    /// Slot exists but holds no capability (→ ERR_INVALID_CAP).
    EmptySlot,
    /// Capability references a different object type
    /// (→ ERR_WRONG_OBJECT_TYPE).
    WrongObjectType,
    /// Held rights do not include the required rights
    /// (→ ERR_INSUFFICIENT_RIGHTS).
    InsufficientRights,
    /// Insertion target already occupied (kernel/boot error).
    SlotOccupied,
}

/// Per-task capability table.
#[derive(Debug)]
pub struct CapTable {
    slots: [Option<Capability>; CAP_TABLE_SLOTS],
}

impl CapTable {
    pub const fn new() -> Self {
        CapTable {
            slots: [None; CAP_TABLE_SLOTS],
        }
    }

    /// Install a capability (kernel-internal; boot-time in v0.1).
    pub fn insert(&mut self, index: usize, cap: Capability) -> Result<(), CapError> {
        if index >= CAP_TABLE_SLOTS {
            return Err(CapError::InvalidIndex);
        }
        if self.slots[index].is_some() {
            return Err(CapError::SlotOccupied);
        }
        self.slots[index] = Some(cap);
        Ok(())
    }

    /// Revoke a capability: the slot becomes empty; later lookups fail
    /// with EmptySlot (docs/03 §8: use after revocation is invalid).
    pub fn revoke(&mut self, index: usize) -> Result<(), CapError> {
        if index >= CAP_TABLE_SLOTS {
            return Err(CapError::InvalidIndex);
        }
        if self.slots[index].take().is_none() {
            return Err(CapError::EmptySlot);
        }
        Ok(())
    }

    /// The enforcement point. Fixed check order (docs/06_CAPABILITY_
    /// MODEL.md §4): bounds → occupancy → object type → rights. Only
    /// full success returns the object reference.
    pub fn lookup(
        &self,
        index: usize,
        expected_type: ObjectType,
        required: Rights,
    ) -> Result<ObjectRef, CapError> {
        if index >= CAP_TABLE_SLOTS {
            return Err(CapError::InvalidIndex);
        }
        let Some(cap) = &self.slots[index] else {
            return Err(CapError::EmptySlot);
        };
        if cap.object().object_type != expected_type {
            return Err(CapError::WrongObjectType);
        }
        if !cap.rights().contains(required) {
            return Err(CapError::InsufficientRights);
        }
        Ok(cap.object())
    }

    /// Inspect a slot without a rights check (sys_cap_query: a task may
    /// query its own authority, docs/04_SYSCALL_MODEL.md).
    pub fn query(&self, index: usize) -> Result<(ObjectType, Rights), CapError> {
        if index >= CAP_TABLE_SLOTS {
            return Err(CapError::InvalidIndex);
        }
        match &self.slots[index] {
            Some(cap) => Ok((cap.object().object_type, cap.rights())),
            None => Err(CapError::EmptySlot),
        }
    }
}

impl Default for CapTable {
    fn default() -> Self {
        Self::new()
    }
}
