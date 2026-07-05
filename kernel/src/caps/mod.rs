//! AxiomRT capability-based access control (Phase 9).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md,
//! docs/02_KERNEL_BLUEPRINT.md §7 (trust boundary).
//!
//! AXIOM-CAP-001 scope: rights and the capability type. The lookup
//! table (the enforcement point) lands with AXIOM-CAP-002; IPC
//! integration with AXIOM-CAP-003. Rule of the phase: **no syscall
//! uses raw object access** — every object reference a syscall
//! receives is a capability-table index.

pub mod capability;
pub mod rights;
pub mod table;

pub use capability::{Capability, ObjectRef, ObjectType};
pub use rights::Rights;
pub use table::{CapError, CapTable, CAP_TABLE_SLOTS};
