//! Sv39 page table construction (AXIOM-MEMHW-003).
//!
//! Requirement reference: docs/12_MMU_SV39.md §2, §4, §5.
//!
//! The page table walk is modeled over an index-based arena of table
//! pages so the mapping logic is host-testable without raw pointers
//! (`translate` mirrors the hardware walk). The on-target wrapper
//! (activation) lives in AXIOM-MEMHW-004; this task builds and verifies
//! the table structure and the kernel mapping.

use crate::memory::pagetable::Permissions;
use crate::memory::sv39::{Pte, PteError};
use crate::memory::{PhysAddr, VirtAddr, PAGE_SIZE};

/// Sv39 entries per table.
pub const ENTRIES: usize = 512;
/// Number of table pages in the arena (root + intermediates + leaves,
/// with headroom for kernel + one user address space in v0.2).
pub const ARENA_TABLES: usize = 16;

/// One 4 KiB page table, 4 KiB aligned (hardware requirement).
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct Table {
    pub entries: [u64; ENTRIES],
}

impl Table {
    pub const fn zeroed() -> Self {
        Table {
            entries: [0; ENTRIES],
        }
    }
}

/// Explicit failure behavior for mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// Virtual or physical address not 4 KiB aligned.
    Misaligned,
    /// The table arena is exhausted.
    OutOfTables,
    /// Illegal PTE encoding (propagated from sv39::Pte).
    Pte(PteError),
    /// A page is already mapped at this virtual address.
    AlreadyMapped,
    /// The walk hit a leaf (megapage) where a table was expected —
    /// not used in v0.2 (4 KiB pages only) but rejected explicitly.
    UnexpectedLeaf,
}

/// An arena of Sv39 tables addressed by index. `base_pa` is the
/// physical address of `tables[0]`; on target it is the static arena's
/// own address (identity mapping), in host tests it is arbitrary.
pub struct Arena<'a> {
    tables: &'a mut [Table],
    used: usize,
    base_pa: u64,
}

const fn vpn(va: u64, level: usize) -> usize {
    ((va >> (12 + level * 9)) & 0x1ff) as usize
}

impl<'a> Arena<'a> {
    /// Create an arena over `tables`, physically based at `base_pa`
    /// (must be 4 KiB aligned). Index 0 is reserved as the root and is
    /// pre-allocated.
    pub fn new(tables: &'a mut [Table], base_pa: u64) -> Self {
        let mut arena = Arena {
            tables,
            used: 0,
            base_pa,
        };
        // Root table = index 0.
        arena.used = 1;
        arena
    }

    /// Physical address of table `idx`.
    pub fn pa_of(&self, idx: usize) -> PhysAddr {
        PhysAddr::new(self.base_pa + (idx as u64) * PAGE_SIZE)
    }

    /// Root table physical address (for satp).
    pub fn root_pa(&self) -> PhysAddr {
        self.pa_of(0)
    }

    /// Number of tables in use.
    pub fn used(&self) -> usize {
        self.used
    }

    fn alloc(&mut self) -> Result<usize, MapError> {
        if self.used >= self.tables.len() {
            return Err(MapError::OutOfTables);
        }
        let idx = self.used;
        self.tables[idx] = Table::zeroed();
        self.used += 1;
        Ok(idx)
    }

    /// Convert a pointer-PTE's PPN back to an arena table index.
    fn table_index_of(&self, pte: Pte) -> usize {
        let ppn = (pte.bits() >> 10) & 0xfff_ffff_ffff;
        ((ppn * PAGE_SIZE - self.base_pa) / PAGE_SIZE) as usize
    }

    /// Map one 4 KiB page `va -> pa` with `perms`. Allocates
    /// intermediate tables as needed. Atomic: on error nothing new is
    /// left half-linked that a translate would use.
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        perms: Permissions,
    ) -> Result<(), MapError> {
        if !va.is_page_aligned() || !pa.is_page_aligned() {
            return Err(MapError::Misaligned);
        }
        let leaf = Pte::leaf(pa, perms).map_err(MapError::Pte)?;

        let mut cur = 0usize; // root
        for level in [2usize, 1usize] {
            let idx = vpn(va.as_u64(), level);
            let entry = Pte::from_bits(self.tables[cur].entries[idx]);
            if entry.is_leaf() {
                return Err(MapError::UnexpectedLeaf);
            }
            if entry.is_valid() {
                cur = self.table_index_of(entry);
            } else {
                let next = self.alloc()?;
                let ptr = Pte::pointer(self.pa_of(next)).map_err(MapError::Pte)?;
                self.tables[cur].entries[idx] = ptr.bits();
                cur = next;
            }
        }
        let idx0 = vpn(va.as_u64(), 0);
        if Pte::from_bits(self.tables[cur].entries[idx0]).is_valid() {
            return Err(MapError::AlreadyMapped);
        }
        self.tables[cur].entries[idx0] = leaf.bits();
        Ok(())
    }

    /// Software walk mirroring the hardware (for tests and page-fault
    /// diagnostics): resolve `va` to (physical address, permissions).
    pub fn translate(&self, va: VirtAddr) -> Option<(PhysAddr, Permissions)> {
        let mut cur = 0usize;
        for level in [2usize, 1usize] {
            let idx = vpn(va.as_u64(), level);
            let entry = Pte::from_bits(self.tables[cur].entries[idx]);
            if !entry.is_valid() || entry.is_leaf() {
                return None;
            }
            cur = self.table_index_of(entry);
        }
        let idx0 = vpn(va.as_u64(), 0);
        let leaf = Pte::from_bits(self.tables[cur].entries[idx0]);
        if !leaf.is_leaf() {
            return None;
        }
        let ppn = (leaf.bits() >> 10) & 0xfff_ffff_ffff;
        let pa = PhysAddr::new(ppn * PAGE_SIZE + (va.as_u64() & (PAGE_SIZE - 1)));
        let perms = Permissions {
            read: leaf.readable(),
            write: leaf.writable(),
            execute: leaf.executable(),
            user: leaf.is_user(),
            device: false,
        };
        Some((pa, perms))
    }

    /// Identity-map a contiguous physical range `[start, end)` (both 4
    /// KiB aligned) with `perms`. Used to build the kernel table.
    pub fn identity_map_range(
        &mut self,
        start: u64,
        end: u64,
        perms: Permissions,
    ) -> Result<(), MapError> {
        let mut addr = start;
        while addr < end {
            self.map_page(VirtAddr::new(addr), PhysAddr::new(addr), perms)?;
            addr += PAGE_SIZE;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perms_krw() -> Permissions {
        Permissions::kernel_rw()
    }

    fn arena_storage() -> [Table; ARENA_TABLES] {
        [Table::zeroed(); ARENA_TABLES]
    }

    #[test]
    fn map_then_translate_round_trip() {
        let mut store = arena_storage();
        let mut a = Arena::new(&mut store, 0x8020_0000);
        let va = VirtAddr::new(0x8020_0000);
        a.map_page(va, PhysAddr::new(0x8020_0000), perms_krw())
            .unwrap();
        let (pa, p) = a.translate(va).expect("mapped");
        assert_eq!(pa, PhysAddr::new(0x8020_0000));
        assert!(p.read && p.write && !p.user);
    }

    #[test]
    fn translate_unmapped_is_none() {
        let mut store = arena_storage();
        let a = Arena::new(&mut store, 0x8020_0000);
        assert!(a.translate(VirtAddr::new(0x8020_0000)).is_none());
    }

    #[test]
    fn offset_within_page_preserved() {
        let mut store = arena_storage();
        let mut a = Arena::new(&mut store, 0x8020_0000);
        a.map_page(VirtAddr::new(0x4000), PhysAddr::new(0x9000), perms_krw())
            .unwrap();
        let (pa, _) = a
            .translate(VirtAddr::new(0x4abc))
            .expect("mapped page covers offset");
        assert_eq!(pa, PhysAddr::new(0x9abc));
    }

    #[test]
    fn double_map_rejected() {
        let mut store = arena_storage();
        let mut a = Arena::new(&mut store, 0x8020_0000);
        a.map_page(VirtAddr::new(0x2000), PhysAddr::new(0x2000), perms_krw())
            .unwrap();
        assert_eq!(
            a.map_page(VirtAddr::new(0x2000), PhysAddr::new(0x3000), perms_krw()),
            Err(MapError::AlreadyMapped)
        );
    }

    #[test]
    fn distinct_gigabyte_regions_get_separate_subtrees() {
        // UART (VPN2=0) and kernel (VPN2=2) exercise two subtrees.
        let mut store = arena_storage();
        let mut a = Arena::new(&mut store, 0x8020_0000);
        a.map_page(
            VirtAddr::new(0x1000_0000),
            PhysAddr::new(0x1000_0000),
            Permissions::kernel_device(),
        )
        .unwrap();
        a.map_page(
            VirtAddr::new(0x8020_0000),
            PhysAddr::new(0x8020_0000),
            perms_krw(),
        )
        .unwrap();
        assert!(a.translate(VirtAddr::new(0x1000_0000)).is_some());
        assert!(a.translate(VirtAddr::new(0x8020_0000)).is_some());
        // root + 2 levels for each of the 2 regions = 1 + 4 = 5 tables.
        assert_eq!(a.used(), 5);
    }

    #[test]
    fn identity_range_maps_every_page() {
        let mut store = arena_storage();
        let mut a = Arena::new(&mut store, 0x8020_0000);
        a.identity_map_range(0x8020_0000, 0x8020_0000 + 4 * PAGE_SIZE, perms_krw())
            .unwrap();
        for i in 0..4 {
            let va = VirtAddr::new(0x8020_0000 + i * PAGE_SIZE);
            assert_eq!(a.translate(va).unwrap().0, PhysAddr::new(va.as_u64()));
        }
    }

    #[test]
    fn arena_exhaustion_is_explicit() {
        let mut store = [Table::zeroed(); 2]; // root + 1: too few for a full path
        let mut a = Arena::new(&mut store, 0x8020_0000);
        assert_eq!(
            a.map_page(
                VirtAddr::new(0x8020_0000),
                PhysAddr::new(0x8020_0000),
                perms_krw()
            ),
            Err(MapError::OutOfTables)
        );
    }
}
