//! AxiomRT user task layer (Phase 7).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md, docs/10_USER_MODE.md.
//!
//! AXIOM-USER-001 scope: the user image *model* only — descriptors that
//! say what a user task is (entry, stack region, address space). No
//! user-mode transition exists in this task; AXIOM-USER-002 adds it.

pub mod image;

pub use image::{ImageError, UserImage};
