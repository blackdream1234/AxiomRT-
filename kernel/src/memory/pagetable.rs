//! Page table model (AXIOM-MEM-003).
//!
//! Requirement reference: docs/05_MEMORY_MODEL.md §5–§6.
//!
//! Model layer only: no MMU activation, no satp writes, no hardware
//! entries. The model is the specification the hardware page table must
//! refine (docs/05 §5): every rule rejected here must be impossible to
//! express in a real Sv39 entry later.
//!
//! Enforced rules (docs/05 core rules; MEM-P1/P2/P5):
//!   * kernel frames can never be mapped USER-accessible
//!   * a frame can only be mapped by the address space that owns it
//!   * only Allocated frames of correct alignment can be mapped
//!   * USER and KERNEL are mutually exclusive; user mappings are W^X
//!   * DEVICE mappings are kernel-only and never EXECUTE (v0.1)
//!   * one virtual page maps at most once per address space
//!
//! Static capacity, no heap: mappings live in a fixed-size array.

use super::frame::{FrameOwner, FrameState, PhysicalFrame};
use super::{AddressSpaceId, PhysAddr, VirtAddr};

/// Maximum mappings per address space in v0.1 (static, no heap).
pub const MAX_MAPPINGS: usize = 64;

/// Memory permissions (docs/05_MEMORY_MODEL.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
    pub device: bool,
}

impl Permissions {
    pub const fn user_rx() -> Self {
        Permissions { read: true, write: false, execute: true, user: true, device: false }
    }
    pub const fn user_rw() -> Self {
        Permissions { read: true, write: true, execute: false, user: true, device: false }
    }
    pub const fn user_r() -> Self {
        Permissions { read: true, write: false, execute: false, user: true, device: false }
    }
    pub const fn kernel_rw() -> Self {
        Permissions { read: true, write: true, execute: false, user: false, device: false }
    }
    pub const fn kernel_device() -> Self {
        Permissions { read: true, write: true, execute: false, user: false, device: true }
    }
}

/// One virtual-page → physical-frame mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    pub vpage: VirtAddr,
    pub frame: PhysAddr,
    pub perms: Permissions,
}

/// Explicit failure behavior (docs/03_KERNEL_OBJECTS.md §5: forbidden
/// entries are rejected before the hardware would see them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// Virtual page or frame base not 4 KiB aligned.
    Misaligned,
    /// Kernel memory can never be USER-mapped (MEM-P1).
    KernelFrameUserMapped,
    /// The frame is not owned by this address space (MEM-P2).
    NotOwner,
    /// Frame must be in state Allocated to be mapped.
    FrameNotMappable,
    /// Virtual address outside the window valid for the mapping kind.
    RangeViolation,
    /// USER mappings must not be WRITE+EXECUTE (MEM-P5).
    WxViolation,
    /// DEVICE mappings are kernel-only and never EXECUTE in v0.1.
    DeviceRuleViolation,
    /// Permissions grant no access at all, or USER==KERNEL confusion.
    InvalidPermissions,
    /// The virtual page is already mapped in this address space.
    AlreadyMapped,
    /// Static mapping table is full (MAX_MAPPINGS).
    TableFull,
    /// Unmap target not present.
    NotMapped,
}

/// Page table model of one address space.
#[derive(Debug)]
pub struct PageTable {
    owner: AddressSpaceId,
    entries: [Option<Mapping>; MAX_MAPPINGS],
}

impl PageTable {
    pub const fn new(owner: AddressSpaceId) -> Self {
        PageTable { owner, entries: [None; MAX_MAPPINGS] }
    }

    pub const fn owner(&self) -> AddressSpaceId {
        self.owner
    }

    /// Number of live mappings.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.is_none())
    }

    /// Look up the mapping covering `vpage` (exact page match).
    pub fn lookup(&self, vpage: VirtAddr) -> Option<&Mapping> {
        self.entries
            .iter()
            .flatten()
            .find(|m| m.vpage == vpage.page_base())
    }

    fn validate_perms(perms: &Permissions) -> Result<(), MapError> {
        if !perms.read && !perms.write && !perms.execute {
            return Err(MapError::InvalidPermissions);
        }
        if perms.user {
            // W^X for user mappings (MEM-P5).
            if perms.write && perms.execute {
                return Err(MapError::WxViolation);
            }
            // DEVICE is kernel-only in v0.1 (docs/05 §7).
            if perms.device {
                return Err(MapError::DeviceRuleViolation);
            }
        }
        if perms.device && perms.execute {
            return Err(MapError::DeviceRuleViolation);
        }
        Ok(())
    }

    /// Insert a mapping. Atomic: on any error the table is unchanged
    /// (docs/03_KERNEL_OBJECTS.md §3 failure behavior). On success the
    /// frame is marked Mapped.
    pub fn map(
        &mut self,
        vpage: VirtAddr,
        frame: &mut PhysicalFrame,
        perms: Permissions,
    ) -> Result<(), MapError> {
        // 1. Alignment.
        if !vpage.is_page_aligned() || !frame.base().is_page_aligned() {
            return Err(MapError::Misaligned);
        }
        // 2. Permission structure.
        Self::validate_perms(&perms)?;
        // 3. Kernel memory is never user-accessible (MEM-P1) — checked
        //    structurally against the physical range, independent of
        //    ownership bookkeeping.
        if perms.user && frame.base().is_kernel() {
            return Err(MapError::KernelFrameUserMapped);
        }
        // 4. User mappings must target the user virtual window; kernel
        //    mappings must not (docs/05 §1: anything not mapped is
        //    inaccessible, and windows are disjoint).
        if perms.user && !vpage.is_user() {
            return Err(MapError::RangeViolation);
        }
        if !perms.user && vpage.is_user() {
            return Err(MapError::RangeViolation);
        }
        // 5. Ownership: only the owning address space may map the frame
        //    (MEM-P2, no sharing in v0.1). Kernel mappings require
        //    kernel-owned frames.
        let owner_ok = if perms.user {
            frame.owner() == FrameOwner::AddressSpace(self.owner)
        } else {
            frame.owner() == FrameOwner::Kernel
        };
        if !owner_ok {
            return Err(MapError::NotOwner);
        }
        // 6. Frame lifecycle: only Allocated frames are mappable (a
        //    Mapped frame is already mapped somewhere — double mapping
        //    is structurally impossible).
        if frame.state() != FrameState::Allocated {
            return Err(MapError::FrameNotMappable);
        }
        // 7. No double-mapping of the virtual page.
        if self.lookup(vpage).is_some() {
            return Err(MapError::AlreadyMapped);
        }
        // 8. Capacity.
        let Some(slot) = self.entries.iter_mut().find(|e| e.is_none()) else {
            return Err(MapError::TableFull);
        };
        // All checks passed: apply fully (atomic success path).
        *slot = Some(Mapping { vpage, frame: frame.base(), perms });
        frame
            .mark_mapped()
            .expect("state checked Allocated above; kernel invariant");
        Ok(())
    }

    /// Remove a mapping and return the frame to Allocated.
    pub fn unmap(
        &mut self,
        vpage: VirtAddr,
        frame: &mut PhysicalFrame,
    ) -> Result<(), MapError> {
        let slot = self
            .entries
            .iter_mut()
            .find(|e| matches!(e, Some(m) if m.vpage == vpage.page_base()));
        let Some(slot) = slot else {
            return Err(MapError::NotMapped);
        };
        let mapping = slot.expect("matched Some above");
        if mapping.frame != frame.base() {
            return Err(MapError::NotMapped);
        }
        frame.mark_unmapped().map_err(|_| MapError::FrameNotMappable)?;
        *slot = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASID: AddressSpaceId = AddressSpaceId(7);
    const OTHER_ASID: AddressSpaceId = AddressSpaceId(9);

    fn user_frame(owner: AddressSpaceId) -> PhysicalFrame {
        let mut f = PhysicalFrame::new_free(PhysAddr::new(0x1000_0000));
        f.allocate(FrameOwner::AddressSpace(owner)).unwrap();
        f
    }

    fn kernel_frame() -> PhysicalFrame {
        let mut f = PhysicalFrame::new_free(PhysAddr::new(0x8030_0000));
        f.allocate(FrameOwner::Kernel).unwrap();
        f
    }

    #[test]
    fn map_and_lookup_user_page() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        pt.map(VirtAddr::new(0x2000), &mut f, Permissions::user_rw()).unwrap();
        assert_eq!(f.state(), FrameState::Mapped);
        let m = pt.lookup(VirtAddr::new(0x2abc)).expect("page covers 0x2abc");
        assert_eq!(m.frame, PhysAddr::new(0x1000_0000));
    }

    #[test]
    fn kernel_frame_never_user_mapped() {
        let mut pt = PageTable::new(ASID);
        // Even if ownership bookkeeping were subverted, the physical
        // range check must reject this (MEM-P1).
        let mut f = kernel_frame();
        assert_eq!(
            pt.map(VirtAddr::new(0x2000), &mut f, Permissions::user_r()),
            Err(MapError::KernelFrameUserMapped)
        );
        assert_eq!(f.state(), FrameState::Allocated, "atomic: no state change on error");
    }

    #[test]
    fn foreign_frame_rejected() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(OTHER_ASID);
        assert_eq!(
            pt.map(VirtAddr::new(0x2000), &mut f, Permissions::user_rw()),
            Err(MapError::NotOwner),
            "no task can map another task's memory (MEM-P2)"
        );
    }

    #[test]
    fn wx_user_mapping_rejected() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        let wx = Permissions { read: true, write: true, execute: true, user: true, device: false };
        assert_eq!(pt.map(VirtAddr::new(0x2000), &mut f, wx), Err(MapError::WxViolation));
    }

    #[test]
    fn device_mapping_is_kernel_only() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        let user_dev = Permissions { device: true, ..Permissions::user_rw() };
        assert_eq!(
            pt.map(VirtAddr::new(0x2000), &mut f, user_dev),
            Err(MapError::DeviceRuleViolation)
        );
    }

    #[test]
    fn user_page_must_be_in_user_window() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        assert_eq!(
            pt.map(VirtAddr::new(0x8020_0000), &mut f, Permissions::user_rw()),
            Err(MapError::RangeViolation)
        );
        assert_eq!(
            pt.map(VirtAddr::new(0x0), &mut f, Permissions::user_rw()),
            Err(MapError::RangeViolation),
            "page zero is never mappable for user tasks"
        );
    }

    #[test]
    fn double_mapping_same_page_rejected() {
        let mut pt = PageTable::new(ASID);
        let mut f1 = user_frame(ASID);
        let mut f2 = PhysicalFrame::new_free(PhysAddr::new(0x1000_1000));
        f2.allocate(FrameOwner::AddressSpace(ASID)).unwrap();
        pt.map(VirtAddr::new(0x2000), &mut f1, Permissions::user_rw()).unwrap();
        assert_eq!(
            pt.map(VirtAddr::new(0x2000), &mut f2, Permissions::user_rw()),
            Err(MapError::AlreadyMapped)
        );
    }

    #[test]
    fn mapped_frame_cannot_be_mapped_twice() {
        let mut pt_a = PageTable::new(ASID);
        let mut pt_b = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        pt_a.map(VirtAddr::new(0x2000), &mut f, Permissions::user_rw()).unwrap();
        assert_eq!(
            pt_b.map(VirtAddr::new(0x3000), &mut f, Permissions::user_rw()),
            Err(MapError::FrameNotMappable),
            "frame state Mapped blocks any second mapping (no sharing)"
        );
    }

    #[test]
    fn misaligned_rejected() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        assert_eq!(
            pt.map(VirtAddr::new(0x2001), &mut f, Permissions::user_rw()),
            Err(MapError::Misaligned)
        );
    }

    #[test]
    fn unmap_returns_frame_to_allocated() {
        let mut pt = PageTable::new(ASID);
        let mut f = user_frame(ASID);
        pt.map(VirtAddr::new(0x2000), &mut f, Permissions::user_rw()).unwrap();
        pt.unmap(VirtAddr::new(0x2000), &mut f).unwrap();
        assert_eq!(f.state(), FrameState::Allocated);
        assert!(pt.is_empty());
        assert_eq!(pt.unmap(VirtAddr::new(0x2000), &mut f), Err(MapError::NotMapped));
    }

    #[test]
    fn table_capacity_is_bounded() {
        let mut pt = PageTable::new(ASID);
        for i in 0..MAX_MAPPINGS {
            let mut f = PhysicalFrame::new_free(PhysAddr::new(0x1000_0000 + (i as u64) * 0x1000));
            f.allocate(FrameOwner::AddressSpace(ASID)).unwrap();
            pt.map(
                VirtAddr::new(0x10_0000 + (i as u64) * 0x1000),
                &mut f,
                Permissions::user_rw(),
            )
            .unwrap();
        }
        let mut extra = PhysicalFrame::new_free(PhysAddr::new(0x1100_0000));
        extra.allocate(FrameOwner::AddressSpace(ASID)).unwrap();
        assert_eq!(
            pt.map(VirtAddr::new(0x20_0000), &mut extra, Permissions::user_rw()),
            Err(MapError::TableFull)
        );
    }
}
