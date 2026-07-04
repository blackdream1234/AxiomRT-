//! AxiomRT memory model (Phase 4).
//!
//! Requirement reference: docs/05_MEMORY_MODEL.md.
//!
//! Phase 4 scope: model layer only — typed addresses, physical frame
//! lifecycle, and the page table model. No page table hardware
//! activation, no allocator, no dynamic allocation (static structures
//! only), no shared memory, no device mappings for user tasks.

pub mod address;

pub use address::{PhysAddr, VirtAddr, PAGE_SIZE};

/// Identifier of an AddressSpace (docs/03_KERNEL_OBJECTS.md §3).
/// v0.1: assigned statically at boot, never reused within a boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressSpaceId(pub u32);
