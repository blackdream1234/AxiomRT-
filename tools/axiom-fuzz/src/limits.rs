//! Hard resource limits for the host fuzz harness.

/// Maximum generated input size accepted by the harness (1 MiB).
pub const MAX_INPUT_LEN: usize = 1024 * 1024;

/// Maximum number of cases accepted in one invocation.
pub const MAX_ITERATIONS: u64 = 10_000_000;

/// Maximum number of regular files loaded from a corpus directory.
pub const MAX_CORPUS_ENTRIES: usize = 4096;

/// Maximum combined bytes retained from a corpus directory (16 MiB).
pub const MAX_CORPUS_BYTES: usize = 16 * 1024 * 1024;
