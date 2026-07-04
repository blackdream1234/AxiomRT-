//! Physical frame model (AXIOM-MEM-002).
//!
//! Requirement reference: docs/05_MEMORY_MODEL.md §4,
//! docs/03_KERNEL_OBJECTS.md §4 (PhysicalFrame).
//!
//! Model layer only: represents the lifecycle and single-owner invariant
//! of one physical frame. No allocator, no heap — frames live in a static
//! pool created at boot (later phase).
//!
//! Invariant (MEM-P4): every frame has exactly one owner at any time.
//! State transitions are total functions with explicit errors; there is
//! no way to move a frame between owners without passing through
//! `free()` (which models the mandatory scrub, docs/05 §4).

use super::{AddressSpaceId, PhysAddr};

/// Who owns a frame. Exactly one owner at any time (MEM-P4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOwner {
    /// In the free pool, owned by no subsystem.
    FreePool,
    /// Kernel image, stacks, page tables, static pools.
    Kernel,
    /// Exactly one user address space (no sharing in v0.1, MEM-P2).
    AddressSpace(AddressSpaceId),
}

/// Frame lifecycle states (docs/03_KERNEL_OBJECTS.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    /// In the free pool, scrubbed, ready for allocation.
    Free,
    /// Allocated to an owner, not yet mapped.
    Allocated,
    /// Mapped into its owner's address space.
    Mapped,
    /// Held after a fault for analysis; never reused this boot
    /// (docs/06_FAULT_MODEL.md, Quarantine).
    Quarantined,
}

/// Explicit failure behavior for invalid lifecycle operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Operation requires state Free.
    NotFree,
    /// Operation requires state Allocated.
    NotAllocated,
    /// Operation requires state Mapped.
    NotMapped,
    /// Frames cannot be allocated directly to the free pool.
    InvalidOwner,
    /// Quarantined frames never leave quarantine within a boot.
    Quarantined,
    /// A mapped frame cannot be freed (unmap first: no dangling
    /// mappings, docs/03_KERNEL_OBJECTS.md §4 invalid operations).
    StillMapped,
}

/// One physical frame (4 KiB, PAGE_SIZE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalFrame {
    base: PhysAddr,
    state: FrameState,
    owner: FrameOwner,
}

impl PhysicalFrame {
    /// A new frame enters the model in the free pool.
    /// Precondition: `base` is frame-aligned (caller uses
    /// `PhysAddr::frame_base`).
    pub const fn new_free(base: PhysAddr) -> Self {
        PhysicalFrame {
            base,
            state: FrameState::Free,
            owner: FrameOwner::FreePool,
        }
    }

    pub const fn base(&self) -> PhysAddr {
        self.base
    }
    pub const fn state(&self) -> FrameState {
        self.state
    }
    pub const fn owner(&self) -> FrameOwner {
        self.owner
    }

    /// Free → Allocated(owner). Boot/setup-time only in v0.1.
    pub fn allocate(&mut self, owner: FrameOwner) -> Result<(), FrameError> {
        if owner == FrameOwner::FreePool {
            return Err(FrameError::InvalidOwner);
        }
        match self.state {
            FrameState::Free => {
                self.state = FrameState::Allocated;
                self.owner = owner;
                Ok(())
            }
            FrameState::Quarantined => Err(FrameError::Quarantined),
            _ => Err(FrameError::NotFree),
        }
    }

    /// Allocated → Mapped (into the owner's address space only; the
    /// cross-check against the mapping address space is enforced by the
    /// page table model, AXIOM-MEM-003).
    pub fn mark_mapped(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Allocated => {
                self.state = FrameState::Mapped;
                Ok(())
            }
            FrameState::Quarantined => Err(FrameError::Quarantined),
            _ => Err(FrameError::NotAllocated),
        }
    }

    /// Mapped → Allocated.
    pub fn mark_unmapped(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Mapped => {
                self.state = FrameState::Allocated;
                Ok(())
            }
            FrameState::Quarantined => Err(FrameError::Quarantined),
            _ => Err(FrameError::NotMapped),
        }
    }

    /// Allocated → Free. Models the mandatory scrub before reuse
    /// (docs/05 §4: no data remanence across tasks). Freeing a mapped
    /// frame is rejected: unmap first.
    pub fn free(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Allocated => {
                self.state = FrameState::Free;
                self.owner = FrameOwner::FreePool;
                Ok(())
            }
            FrameState::Mapped => Err(FrameError::StillMapped),
            FrameState::Quarantined => Err(FrameError::Quarantined),
            FrameState::Free => Err(FrameError::NotAllocated),
        }
    }

    /// Any owned state → Quarantined (terminal within a boot).
    pub fn quarantine(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Allocated | FrameState::Mapped => {
                self.state = FrameState::Quarantined;
                Ok(())
            }
            FrameState::Quarantined => Err(FrameError::Quarantined),
            FrameState::Free => Err(FrameError::NotAllocated),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PhysicalFrame {
        PhysicalFrame::new_free(PhysAddr::new(0x8100_0000))
    }

    #[test]
    fn lifecycle_happy_path() {
        let mut f = frame();
        assert_eq!(f.state(), FrameState::Free);
        assert_eq!(f.owner(), FrameOwner::FreePool);

        f.allocate(FrameOwner::AddressSpace(AddressSpaceId(1))).unwrap();
        assert_eq!(f.state(), FrameState::Allocated);
        f.mark_mapped().unwrap();
        assert_eq!(f.state(), FrameState::Mapped);
        f.mark_unmapped().unwrap();
        f.free().unwrap();
        assert_eq!(f.owner(), FrameOwner::FreePool);
    }

    #[test]
    fn double_allocate_rejected() {
        let mut f = frame();
        f.allocate(FrameOwner::Kernel).unwrap();
        assert_eq!(
            f.allocate(FrameOwner::AddressSpace(AddressSpaceId(2))),
            Err(FrameError::NotFree),
            "a frame can never move to a second owner without free()"
        );
    }

    #[test]
    fn free_pool_is_not_an_allocation_owner() {
        let mut f = frame();
        assert_eq!(f.allocate(FrameOwner::FreePool), Err(FrameError::InvalidOwner));
    }

    #[test]
    fn mapped_frame_cannot_be_freed() {
        let mut f = frame();
        f.allocate(FrameOwner::AddressSpace(AddressSpaceId(1))).unwrap();
        f.mark_mapped().unwrap();
        assert_eq!(f.free(), Err(FrameError::StillMapped));
    }

    #[test]
    fn double_free_rejected() {
        let mut f = frame();
        f.allocate(FrameOwner::Kernel).unwrap();
        f.free().unwrap();
        assert_eq!(f.free(), Err(FrameError::NotAllocated));
    }

    #[test]
    fn quarantine_is_terminal() {
        let mut f = frame();
        f.allocate(FrameOwner::AddressSpace(AddressSpaceId(3))).unwrap();
        f.quarantine().unwrap();
        assert_eq!(f.free(), Err(FrameError::Quarantined));
        assert_eq!(f.mark_mapped(), Err(FrameError::Quarantined));
        assert_eq!(f.allocate(FrameOwner::Kernel), Err(FrameError::Quarantined));
    }
}
