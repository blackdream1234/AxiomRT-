//! AxiomRT runtime monitoring (Phase 11).
//!
//! Requirement reference: docs/11_RUNTIME_MONITORING.md.
//!
//! Structured evidence events. AXIOM-MON-001: the event model.
//! AXIOM-MON-002: serial export (structured text over the QEMU UART).
//! No storage backend, no filesystem (docs/01 non-goals): events leave
//! the system through the serial port only in v0.1.

pub mod event;

pub use event::{KernelPhase, MonitorEvent, MonitorEventType};
