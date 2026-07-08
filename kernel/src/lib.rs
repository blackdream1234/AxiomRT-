//! AxiomRT kernel library.
//!
//! Formally specified microkernel-based safety runtime for high-assurance
//! embedded systems. Requirement reference: docs/02_KERNEL_BLUEPRINT.md.
//!
//! `no_std`, no heap, no external dependencies. Host-side unit tests build
//! this library with `std` available (test configuration only).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]

// Kernel panic handler: only for the bare-metal RISC-V build. Host test
// builds use the standard library's handler.
#[cfg(all(not(test), target_arch = "riscv64"))]
mod panic;

// Memory model (Phase 4, docs/05_MEMORY_MODEL.md).
pub mod memory;

// Thread model (Phase 5, docs/03_KERNEL_OBJECTS.md §2).
pub mod thread;

// Fixed-priority scheduler (Phase 6, docs/09_SCHEDULER_MODEL.md).
pub mod sched;

// Synchronous IPC (Phase 8, docs/08_IPC_MODEL.md).
pub mod ipc;

// Capability-based access control (Phase 9, docs/06_CAPABILITY_MODEL.md).
pub mod caps;

// Fault events and recovery model (Phase 10, docs/06_FAULT_MODEL.md).
pub mod fault;

// Device objects and device capabilities (Phase v1.5,
// docs/31_USER_SPACE_DRIVER_FRAMEWORK.md).
pub mod device;

// Restricted app image mapping admission (Phase v1.6,
// docs/32_RESTRICTED_APP_IMAGE_FORMAT.md §7).
pub mod loader;

// Runtime monitoring events (Phase 11, docs/11_RUNTIME_MONITORING.md).
pub mod monitor;
