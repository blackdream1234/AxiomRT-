//! AxiomRT synchronous IPC (Phase 8).
//!
//! Requirement reference: docs/08_IPC_MODEL.md,
//! docs/02_KERNEL_BLUEPRINT.md §9 (IPC principle).
//!
//! Synchronous, bounded, copy-based. No shared memory. No buffering
//! beyond the single in-flight rendezvous message. AXIOM-IPC-001 scope:
//! object model (Endpoint, Message, states). Send/receive logic:
//! AXIOM-IPC-002. Capability integration: Phase 9 (AXIOM-CAP-003) —
//! nothing here bypasses capability checks; the syscall layer will
//! perform them before reaching this model.

pub mod endpoint;
pub mod message;

pub use endpoint::{Endpoint, EndpointId, EndpointState};
pub use message::{Message, MessageError, MessageHeader, MSG_MAX_BYTES};
