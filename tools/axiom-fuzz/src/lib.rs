//! Deterministic, bounded host fuzz infrastructure for AxiomRT.
//!
//! This crate is infrastructure only. Protocol and runtime targets are added
//! by later AXIOM-ROBUST tasks.

#![forbid(unsafe_code)]

pub mod case;
pub mod corpus;
pub mod engine;
pub mod failure;
pub mod generator;
pub mod limits;
pub mod result;
pub mod targets;

pub use case::{FuzzCase, MutationKind};
pub use corpus::{Corpus, CorpusEntry};
pub use engine::{Engine, FuzzTarget, RunConfig};
pub use failure::FailureArtifact;
pub use generator::CaseGenerator;
pub use result::{CaseResult, ResultClass, RunSummary};
