//! Fixed deterministic input generator.
//!
//! SplitMix64 is used as a small, stable testing PRNG. It is not
//! cryptographically secure and never draws from the operating system.

use crate::case::{FuzzCase, MutationKind};
use crate::corpus::{Corpus, CorpusEntry};
use crate::limits::MAX_INPUT_LEN;

const SPLITMIX_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(SPLITMIX_GAMMA);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn bounded_usize(&mut self, inclusive_maximum: usize) -> usize {
        if inclusive_maximum == 0 {
            0
        } else {
            (self.next_u64() % (inclusive_maximum as u64 + 1)) as usize
        }
    }
}

/// Stateful generator. Ascending iteration calls are deterministic for a
/// fixed target, seed, maximum length, and corpus.
#[derive(Clone, Debug)]
pub struct CaseGenerator {
    target: String,
    seed: u64,
    max_len: usize,
    corpus: Corpus,
    random: SplitMix64,
}

impl CaseGenerator {
    pub fn new(
        target: impl Into<String>,
        seed: u64,
        max_len: usize,
        corpus: Corpus,
    ) -> Result<Self, String> {
        let target = target.into();
        if target.is_empty() {
            return Err("fuzz target must not be empty".to_string());
        }
        if max_len > MAX_INPUT_LEN {
            return Err(format!(
                "max_len {max_len} exceeds hard limit {MAX_INPUT_LEN}"
            ));
        }
        Ok(Self {
            target,
            seed,
            max_len,
            corpus,
            random: SplitMix64::new(seed),
        })
    }

    pub fn case(&mut self, iteration: u64) -> FuzzCase {
        let (input, mutation, corpus_origin) = match iteration % 8 {
            0 => (Vec::new(), MutationKind::Empty, None),
            1 => (
                self.random_bytes(self.max_len.min(1)),
                MutationKind::BoundaryMinimum,
                None,
            ),
            2 => (
                self.random_bytes(self.max_len),
                MutationKind::BoundaryMaximum,
                None,
            ),
            3 => {
                let length = self.random.bounded_usize(self.max_len);
                (self.random_bytes(length), MutationKind::RandomBytes, None)
            }
            4 => self.bit_flip(),
            5 => self.byte_insert(),
            6 => self.corpus_xor(),
            _ => self.corpus_truncate(),
        };

        FuzzCase {
            target: self.target.clone(),
            seed: self.seed,
            iteration,
            input,
            mutation,
            corpus_origin,
        }
    }

    fn random_bytes(&mut self, length: usize) -> Vec<u8> {
        (0..length).map(|_| self.random.next_u64() as u8).collect()
    }

    fn bit_flip(&mut self) -> (Vec<u8>, MutationKind, Option<String>) {
        let length = self.random.bounded_usize(self.max_len);
        let mut input = self.random_bytes(length);
        if !input.is_empty() {
            let index = self.random.bounded_usize(input.len() - 1);
            let bit = (self.random.next_u64() % 8) as u32;
            input[index] ^= 1u8 << bit;
        }
        (input, MutationKind::BitFlip, None)
    }

    fn byte_insert(&mut self) -> (Vec<u8>, MutationKind, Option<String>) {
        if self.max_len == 0 {
            return (Vec::new(), MutationKind::ByteInsert, None);
        }
        let original_length = self.random.bounded_usize(self.max_len - 1);
        let mut input = self.random_bytes(original_length);
        let index = self.random.bounded_usize(input.len());
        input.insert(index, self.random.next_u64() as u8);
        (input, MutationKind::ByteInsert, None)
    }

    fn corpus_xor(&mut self) -> (Vec<u8>, MutationKind, Option<String>) {
        let Some(entry) = self.select_corpus_entry() else {
            let length = self.random.bounded_usize(self.max_len);
            return (self.random_bytes(length), MutationKind::RandomBytes, None);
        };
        let mut input = entry.bytes;
        if !input.is_empty() {
            let index = self.random.bounded_usize(input.len() - 1);
            input[index] ^= self.random.next_u64() as u8;
        }
        (input, MutationKind::CorpusXor, Some(entry.origin))
    }

    fn corpus_truncate(&mut self) -> (Vec<u8>, MutationKind, Option<String>) {
        let Some(entry) = self.select_corpus_entry() else {
            let length = self.random.bounded_usize(self.max_len);
            return (self.random_bytes(length), MutationKind::RandomBytes, None);
        };
        let mut input = entry.bytes;
        let new_length = self.random.bounded_usize(input.len());
        input.truncate(new_length);
        (input, MutationKind::CorpusTruncate, Some(entry.origin))
    }

    fn select_corpus_entry(&mut self) -> Option<CorpusEntry> {
        if self.corpus.is_empty() {
            return None;
        }
        let index = self.random.bounded_usize(self.corpus.len() - 1);
        self.corpus.entries().get(index).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseGenerator, SplitMix64};
    use crate::{Corpus, CorpusEntry};

    fn cases(seed: u64, max_len: usize, corpus: Corpus, count: u64) -> Vec<crate::FuzzCase> {
        let mut generator =
            CaseGenerator::new("smoke", seed, max_len, corpus).expect("valid generator");
        (0..count)
            .map(|iteration| generator.case(iteration))
            .collect()
    }

    #[test]
    fn splitmix64_reference_sequence_is_fixed() {
        let mut random = SplitMix64::new(0);
        assert_eq!(random.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(random.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn same_seed_produces_same_cases_and_bytes() {
        let left = cases(42, 64, Corpus::empty(), 32);
        let right = cases(42, 64, Corpus::empty(), 32);
        assert_eq!(left, right);
    }

    #[test]
    fn different_seed_changes_generated_sequence() {
        let left = cases(42, 64, Corpus::empty(), 32);
        let right = cases(43, 64, Corpus::empty(), 32);
        assert_ne!(left, right);
        assert!(left
            .iter()
            .zip(right.iter())
            .any(|(left, right)| left.input != right.input));
    }

    #[test]
    fn maximum_length_is_always_respected() {
        for max_len in [0, 1, 2, 31, 128] {
            for case in cases(99, max_len, Corpus::empty(), 128) {
                assert!(case.input_len() <= max_len);
            }
        }
    }

    #[test]
    fn empty_and_zero_length_corpus_inputs_do_not_panic() {
        let corpus = Corpus::from_entries(vec![CorpusEntry::new("empty.bin", Vec::new())], 0)
            .expect("zero-length corpus entry");
        let generated = cases(7, 0, corpus, 16);
        assert!(generated.iter().all(|case| case.input.is_empty()));
    }

    #[test]
    fn small_corpus_drives_named_mutations() {
        let corpus = Corpus::from_entries(vec![CorpusEntry::new("seed.bin", vec![1, 2, 3])], 8)
            .expect("small corpus");
        let generated = cases(7, 8, corpus, 8);
        assert_eq!(generated[6].corpus_origin.as_deref(), Some("seed.bin"));
        assert_eq!(generated[7].corpus_origin.as_deref(), Some("seed.bin"));
    }
}
