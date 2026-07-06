//! axiomctl library surface.
//!
//! Requirement reference: docs/23_AXIOMCTL_CLI.md, docs/24_STUDIO.md §3.
//! Shared between the axiomctl binary and AxiomRT Studio so the CLI
//! and the dashboard parse logs and locate the repository with exactly
//! the same code — they cannot diverge.

pub mod events;

use std::env;
use std::path::PathBuf;

/// Walk up from the current directory to the repository root,
/// identified by `scripts/verify_all.sh` next to `kernel/Cargo.toml`.
pub fn repo_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        if dir.join("scripts/verify_all.sh").is_file() && dir.join("kernel/Cargo.toml").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
