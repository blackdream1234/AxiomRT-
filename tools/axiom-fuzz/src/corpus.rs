//! Deterministic, non-recursive corpus loading.

use crate::limits::{MAX_CORPUS_BYTES, MAX_CORPUS_ENTRIES, MAX_INPUT_LEN};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

/// One named corpus input. The origin is the file name, not an absolute path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusEntry {
    pub origin: String,
    pub bytes: Vec<u8>,
}

impl CorpusEntry {
    pub fn new(origin: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            origin: origin.into(),
            bytes,
        }
    }
}

/// A corpus sorted by origin, with bounded entry count and retained bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Corpus {
    entries: Vec<CorpusEntry>,
}

impl Corpus {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: Vec<CorpusEntry>, max_len: usize) -> io::Result<Self> {
        validate_max_len(max_len)?;
        if entries.len() > MAX_CORPUS_ENTRIES {
            return Err(invalid_data(format!(
                "corpus contains {} entries; limit is {MAX_CORPUS_ENTRIES}",
                entries.len()
            )));
        }

        let mut total = 0usize;
        for entry in &entries {
            if entry.bytes.len() > max_len {
                return Err(invalid_data(format!(
                    "corpus entry {:?} is {} bytes; max_len is {max_len}",
                    entry.origin,
                    entry.bytes.len()
                )));
            }
            total = total
                .checked_add(entry.bytes.len())
                .ok_or_else(|| invalid_data("corpus byte count overflow"))?;
            if total > MAX_CORPUS_BYTES {
                return Err(invalid_data(format!(
                    "corpus retains {total} bytes; limit is {MAX_CORPUS_BYTES}"
                )));
            }
        }

        let mut entries = entries;
        entries.sort_by(|left, right| left.origin.cmp(&right.origin));
        Ok(Self { entries })
    }

    /// Load only regular files directly inside path, sorted by file name.
    /// Files larger than max_len are rejected; they are never truncated.
    pub fn load(path: &Path, max_len: usize) -> io::Result<Self> {
        validate_max_len(max_len)?;
        let mut files = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files.push((entry.file_name(), entry.path()));
            }
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));

        if files.len() > MAX_CORPUS_ENTRIES {
            return Err(invalid_data(format!(
                "corpus contains {} files; limit is {MAX_CORPUS_ENTRIES}",
                files.len()
            )));
        }

        let mut entries = Vec::with_capacity(files.len());
        let mut total = 0usize;
        for (file_name, path) in files {
            let mut bytes = Vec::new();
            File::open(&path)?
                .take(max_len as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > max_len {
                return Err(invalid_data(format!(
                    "corpus file {:?} exceeds max_len {max_len}; oversized files are rejected",
                    file_name
                )));
            }

            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| invalid_data("corpus byte count overflow"))?;
            if total > MAX_CORPUS_BYTES {
                return Err(invalid_data(format!(
                    "corpus retains {total} bytes; limit is {MAX_CORPUS_BYTES}"
                )));
            }

            entries.push(CorpusEntry::new(file_name.to_string_lossy(), bytes));
        }

        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[CorpusEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn validate_max_len(max_len: usize) -> io::Result<()> {
    if max_len > MAX_INPUT_LEN {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("max_len {max_len} exceeds hard limit {MAX_INPUT_LEN}"),
        ))
    } else {
        Ok(())
    }
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::{Corpus, CorpusEntry};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "axiom-fuzz-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn empty_corpus_is_valid() {
        assert!(Corpus::from_entries(Vec::new(), 8)
            .expect("empty corpus")
            .is_empty());
    }

    #[test]
    fn in_memory_corpus_is_sorted() {
        let corpus = Corpus::from_entries(
            vec![
                CorpusEntry::new("z-last", vec![2]),
                CorpusEntry::new("a-first", vec![1]),
            ],
            8,
        )
        .expect("small corpus");
        let origins: Vec<&str> = corpus
            .entries()
            .iter()
            .map(|entry| entry.origin.as_str())
            .collect();
        assert_eq!(origins, ["a-first", "z-last"]);
    }

    #[test]
    fn directory_ordering_is_stable_and_non_recursive() {
        let directory = TempDirectory::new("corpus-order");
        fs::write(directory.0.join("z.bin"), [3]).expect("write z");
        fs::write(directory.0.join("a.bin"), [1]).expect("write a");
        fs::create_dir(directory.0.join("nested")).expect("create nested");
        fs::write(directory.0.join("nested/ignored.bin"), [9]).expect("write nested");

        let corpus = Corpus::load(&directory.0, 8).expect("load corpus");
        let origins: Vec<&str> = corpus
            .entries()
            .iter()
            .map(|entry| entry.origin.as_str())
            .collect();
        assert_eq!(origins, ["a.bin", "z.bin"]);
    }

    #[test]
    fn oversized_file_is_rejected_not_truncated() {
        let directory = TempDirectory::new("corpus-oversized");
        fs::write(directory.0.join("large.bin"), [0, 1, 2]).expect("write corpus file");
        let error = Corpus::load(&directory.0, 2).expect_err("oversized file must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
