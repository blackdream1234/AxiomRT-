//! Sv39 page table entry model (AXIOM-MEMHW-002).
//!
//! Requirement reference: docs/12_MMU_SV39.md §3.
//!
//! The PTE encoding is the hardware realization of the permission rules
//! in docs/05_MEMORY_MODEL.md §6. The constructor rejects every illegal
//! combination (W without R, USER+KERNEL confusion, user W^X violation)
//! so a forbidden entry cannot be built — the model rule becomes a
//! hardware-bit rule. Pure data: host-testable.

use crate::memory::pagetable::Permissions;
use crate::memory::PhysAddr;

/// PTE flag bits (docs/12_MMU_SV39.md §3).
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
pub const PTE_U: u64 = 1 << 4;
pub const PTE_G: u64 = 1 << 5;
pub const PTE_A: u64 = 1 << 6;
pub const PTE_D: u64 = 1 << 7;

/// satp MODE field value selecting Sv39.
pub const SATP_MODE_SV39: u64 = 8 << 60;

/// A single Sv39 page table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pte(u64);

/// Explicit failure behavior for PTE construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PteError {
    /// Physical frame base is not 4 KiB aligned.
    Misaligned,
    /// Leaf grants no access (R=W=X=0) — that encoding is a pointer,
    /// not a leaf.
    NoAccess,
    /// W set without R (reserved/illegal in RISC-V).
    WriteWithoutRead,
    /// User mapping that is both writable and executable (MEM-P5).
    UserWx,
    /// DEVICE/EXECUTE combination is forbidden (docs/05 §6/§7).
    DeviceExecute,
}

impl Pte {
    /// An empty (invalid) entry.
    pub const fn empty() -> Self {
        Pte(0)
    }

    /// Reinterpret raw PTE bits read back from a table (page-table walk).
    pub const fn from_bits(bits: u64) -> Self {
        Pte(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 & PTE_V != 0
    }

    /// A leaf has at least one of R/W/X; a valid entry without them is a
    /// pointer to the next level.
    pub const fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & (PTE_R | PTE_W | PTE_X)) != 0
    }

    /// Build a pointer (non-leaf) PTE referencing a next-level table.
    pub fn pointer(next_table: PhysAddr) -> Result<Self, PteError> {
        if !next_table.is_page_aligned() {
            return Err(PteError::Misaligned);
        }
        Ok(Pte(PTE_V | ppn_field(next_table)))
    }

    /// Build a leaf PTE for `frame` with `perms`. Rejects every illegal
    /// combination so hardware never sees a forbidden entry
    /// (docs/12_MMU_SV39.md §3).
    pub fn leaf(frame: PhysAddr, perms: Permissions) -> Result<Self, PteError> {
        if !frame.is_page_aligned() {
            return Err(PteError::Misaligned);
        }
        if !perms.read && !perms.write && !perms.execute {
            return Err(PteError::NoAccess);
        }
        if perms.write && !perms.read {
            return Err(PteError::WriteWithoutRead);
        }
        if perms.user && perms.write && perms.execute {
            return Err(PteError::UserWx);
        }
        if perms.device && perms.execute {
            return Err(PteError::DeviceExecute);
        }

        let mut bits = PTE_V | ppn_field(frame);
        if perms.read {
            bits |= PTE_R;
        }
        if perms.write {
            bits |= PTE_W;
        }
        if perms.execute {
            bits |= PTE_X;
        }
        if perms.user {
            bits |= PTE_U;
        }
        // Pre-set A/D for deterministic behavior (docs/12 §3).
        bits |= PTE_A | PTE_D;
        Ok(Pte(bits))
    }

    pub const fn is_user(self) -> bool {
        self.0 & PTE_U != 0
    }
    pub const fn readable(self) -> bool {
        self.0 & PTE_R != 0
    }
    pub const fn writable(self) -> bool {
        self.0 & PTE_W != 0
    }
    pub const fn executable(self) -> bool {
        self.0 & PTE_X != 0
    }
}

/// PPN field: physical page number shifted into PTE bits [10:53].
const fn ppn_field(addr: PhysAddr) -> u64 {
    (addr.as_u64() >> 12) << 10
}

/// Compose a satp value for a root table at `root` (Sv39, ASID 0).
pub fn satp_for(root: PhysAddr) -> u64 {
    SATP_MODE_SV39 | (root.as_u64() >> 12)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PhysAddr {
        PhysAddr::new(0x8030_0000)
    }

    #[test]
    fn kernel_rx_leaf_has_no_user_bit() {
        let pte = Pte::leaf(
            frame(),
            Permissions { read: true, write: false, execute: true, user: false, device: false },
        )
        .unwrap();
        assert!(pte.is_leaf());
        assert!(!pte.is_user(), "kernel mapping must not carry U (MEM-P1)");
        assert!(pte.readable() && pte.executable() && !pte.writable());
    }

    #[test]
    fn user_rw_leaf_sets_user_bit() {
        let pte = Pte::leaf(frame(), Permissions::user_rw()).unwrap();
        assert!(pte.is_user());
        assert!(pte.readable() && pte.writable() && !pte.executable());
    }

    #[test]
    fn user_wx_rejected() {
        let wx = Permissions { read: true, write: true, execute: true, user: true, device: false };
        assert_eq!(Pte::leaf(frame(), wx), Err(PteError::UserWx));
    }

    #[test]
    fn write_without_read_rejected() {
        let w = Permissions { read: false, write: true, execute: false, user: false, device: false };
        assert_eq!(Pte::leaf(frame(), w), Err(PteError::WriteWithoutRead));
    }

    #[test]
    fn no_access_leaf_rejected() {
        let none =
            Permissions { read: false, write: false, execute: false, user: true, device: false };
        assert_eq!(Pte::leaf(frame(), none), Err(PteError::NoAccess));
    }

    #[test]
    fn misaligned_rejected() {
        assert_eq!(Pte::leaf(PhysAddr::new(0x8030_0001), Permissions::user_rw()), Err(PteError::Misaligned));
        assert_eq!(Pte::pointer(PhysAddr::new(0x8030_0001)), Err(PteError::Misaligned));
    }

    #[test]
    fn pointer_is_not_a_leaf() {
        let p = Pte::pointer(frame()).unwrap();
        assert!(p.is_valid());
        assert!(!p.is_leaf(), "pointer PTE has R=W=X=0");
    }

    #[test]
    fn ppn_and_satp_encoding() {
        let pte = Pte::leaf(frame(), Permissions::user_rw()).unwrap();
        // PPN of 0x8030_0000 is 0x80300; sits at bits [10:53].
        assert_eq!((pte.bits() >> 10) & 0xfff_ffff_ffff, 0x8_0300);
        let satp = satp_for(PhysAddr::new(0x8020_0000));
        assert_eq!(satp >> 60, 8, "MODE = Sv39");
        assert_eq!(satp & 0xfff_ffff_ffff, 0x8_0200);
    }

    #[test]
    fn device_execute_rejected() {
        let dx = Permissions { read: true, write: false, execute: true, user: false, device: true };
        assert_eq!(Pte::leaf(frame(), dx), Err(PteError::DeviceExecute));
    }
}
