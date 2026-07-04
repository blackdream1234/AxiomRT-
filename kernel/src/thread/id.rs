//! Thread identity (AXIOM-THREAD-001).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md §2.
//!
//! Kernel-assigned, never reused within a boot session. A ThreadId is
//! pure identity: knowing an ID grants no authority (all access goes
//! through capabilities, docs/03 §8).

/// Unique thread identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadId(pub u32);

impl ThreadId {
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}
