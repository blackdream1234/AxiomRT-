//! Capability type (AXIOM-CAP-001).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md §1,
//! docs/03_KERNEL_OBJECTS.md §8 (Capability).
//!
//! A capability is (object reference, object type, rights). It is the
//! only way any task reaches a protected object: no syscall operates on
//! raw object IDs or pointers — every access resolves through a
//! capability table lookup (AXIOM-CAP-002) that checks type and rights
//! before the object is touched.
//!
//! Unforgeability: capabilities live exclusively in kernel memory
//! (capability tables). User code holds only table *indexes*; no
//! capability bits ever cross the user/kernel boundary
//! (docs/03 §8 invalid operations).

use super::rights::Rights;

/// Types of protected kernel objects a capability can reference
/// (docs/03_KERNEL_OBJECTS.md; Project Description §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Thread,
    Endpoint,
    AddressSpace,
    PhysicalFrame,
    Timer,
    SchedulingContext,
    /// Supervisor fault channel (sys_fault_ack, docs/04).
    FaultChannel,
}

/// Reference to one protected object: type tag + kernel object ID.
/// The type tag makes type confusion structurally detectable at
/// lookup time (docs/03 §1 security impact).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectRef {
    pub object_type: ObjectType,
    pub object_id: u32,
}

/// An explicit, kernel-held authority token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    object: ObjectRef,
    rights: Rights,
}

impl Capability {
    /// Mint a capability. Kernel-internal: in v0.1 all capabilities are
    /// minted at boot from the static task descriptions
    /// (docs/03 §8 lifecycle).
    pub const fn new(object: ObjectRef, rights: Rights) -> Self {
        Capability { object, rights }
    }

    pub const fn object(&self) -> ObjectRef {
        self.object
    }

    pub const fn rights(&self) -> Rights {
        self.rights
    }

    /// Derive a weaker capability for the same object (rights can only
    /// shrink; amplification does not exist as an operation).
    pub const fn derive_diminished(&self, removed: Rights) -> Capability {
        Capability { object: self.object, rights: self.rights.diminish(removed) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_carries_object_and_rights() {
        let c = Capability::new(
            ObjectRef { object_type: ObjectType::Endpoint, object_id: 3 },
            Rights::SEND,
        );
        assert_eq!(c.object().object_type, ObjectType::Endpoint);
        assert_eq!(c.object().object_id, 3);
        assert!(c.rights().contains(Rights::SEND));
        assert!(!c.rights().contains(Rights::RECEIVE));
    }

    #[test]
    fn derivation_only_diminishes() {
        let full = Capability::new(
            ObjectRef { object_type: ObjectType::Endpoint, object_id: 1 },
            Rights::SEND.union(Rights::RECEIVE).union(Rights::GRANT),
        );
        let weak = full.derive_diminished(Rights::GRANT.union(Rights::RECEIVE));
        assert!(weak.rights().contains(Rights::SEND));
        assert!(!weak.rights().contains(Rights::GRANT));
        assert_eq!(weak.object(), full.object(), "same object, weaker authority");
    }
}
