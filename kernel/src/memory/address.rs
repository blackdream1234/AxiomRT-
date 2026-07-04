//! Typed addresses and address range constants (AXIOM-MEM-001).
//!
//! Requirement reference: docs/05_MEMORY_MODEL.md §1–§3.
//!
//! `VirtAddr` and `PhysAddr` are distinct wrapper types so that a virtual
//! address can never be passed where a physical address is expected (and
//! vice versa). There is no implicit conversion in either direction and
//! no arithmetic operators: address math is explicit and named.

/// Page size: 4 KiB only in v0.1 (docs/05_MEMORY_MODEL.md §9: no huge
/// pages).
pub const PAGE_SIZE: u64 = 4096;

/// Kernel physical/virtual load region (identity view before MMU
/// activation): OpenSBI hands over at 0x8020_0000; the kernel image,
/// stacks, and static pools live below KERNEL_RANGE_END
/// (docs/05_MEMORY_MODEL.md §2; kernel/linker.ld KERNEL_BASE).
pub const KERNEL_RANGE_START: u64 = 0x8020_0000;
/// End of the kernel-reserved region (exclusive): 0x8800_0000 with the
/// default 128 MiB QEMU virt RAM configuration.
pub const KERNEL_RANGE_END: u64 = 0x8800_0000;

/// User virtual address window (Sv39 lower range,
/// docs/05_MEMORY_MODEL.md §3). Page zero is never mapped so that null
/// dereferences always fault.
pub const USER_RANGE_START: u64 = 0x0000_1000;
/// End of the user virtual window (exclusive).
pub const USER_RANGE_END: u64 = 0x4000_0000;

/// A virtual address (interpretation depends on an AddressSpace).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(u64);

/// A physical address (RAM or device MMIO).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(u64);

impl VirtAddr {
    pub const fn new(value: u64) -> Self {
        VirtAddr(value)
    }
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    pub const fn is_page_aligned(self) -> bool {
        self.0 % PAGE_SIZE == 0
    }
    /// Round down to the containing page boundary.
    pub const fn page_base(self) -> Self {
        VirtAddr(self.0 - (self.0 % PAGE_SIZE))
    }
    /// True if the address lies in the user window
    /// (docs/05_MEMORY_MODEL.md §3).
    pub const fn is_user(self) -> bool {
        self.0 >= USER_RANGE_START && self.0 < USER_RANGE_END
    }
}

impl PhysAddr {
    pub const fn new(value: u64) -> Self {
        PhysAddr(value)
    }
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    pub const fn is_page_aligned(self) -> bool {
        self.0 % PAGE_SIZE == 0
    }
    /// Round down to the containing frame boundary.
    pub const fn frame_base(self) -> Self {
        PhysAddr(self.0 - (self.0 % PAGE_SIZE))
    }
    /// True if the address lies in the kernel-reserved region
    /// (docs/05_MEMORY_MODEL.md §2). Used to structurally reject mapping
    /// kernel frames into user address spaces.
    pub const fn is_kernel(self) -> bool {
        self.0 >= KERNEL_RANGE_START && self.0 < KERNEL_RANGE_END
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_types_prevent_confusion() {
        // Compile-time property: VirtAddr and PhysAddr are different
        // types with no cross-conversion; this test documents the intent
        // and checks the accessors round-trip.
        let v = VirtAddr::new(0x2000);
        let p = PhysAddr::new(0x8020_0000);
        assert_eq!(v.as_u64(), 0x2000);
        assert_eq!(p.as_u64(), 0x8020_0000);
    }

    #[test]
    fn page_alignment() {
        assert!(VirtAddr::new(0x3000).is_page_aligned());
        assert!(!VirtAddr::new(0x3001).is_page_aligned());
        assert_eq!(VirtAddr::new(0x3fff).page_base(), VirtAddr::new(0x3000));
        assert_eq!(PhysAddr::new(0x8020_0123).frame_base(), PhysAddr::new(0x8020_0000));
    }

    #[test]
    fn kernel_and_user_ranges_are_disjoint() {
        assert!(USER_RANGE_END <= KERNEL_RANGE_START);
        assert!(PhysAddr::new(KERNEL_RANGE_START).is_kernel());
        assert!(!PhysAddr::new(KERNEL_RANGE_END).is_kernel());
        assert!(VirtAddr::new(USER_RANGE_START).is_user());
        assert!(!VirtAddr::new(0x0).is_user(), "page zero is never user memory");
        assert!(!VirtAddr::new(USER_RANGE_END).is_user());
        assert!(!VirtAddr::new(KERNEL_RANGE_START).is_user());
    }
}
