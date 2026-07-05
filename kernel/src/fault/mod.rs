//! AxiomRT fault handling (Phase 10).
//!
//! Requirement reference: docs/06_FAULT_MODEL.md.
//!
//! AXIOM-FAULT-001 scope: the structured FaultEvent model only.
//! Handling policy (thread containment, kernel panic path) arrives with
//! AXIOM-FAULT-002; the supervisor notification path with
//! AXIOM-FAULT-003.

pub mod event;

pub use event::{EventState, FaultEvent, FaultType, IllegalEventTransition, Severity};
