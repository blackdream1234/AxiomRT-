//! Harmless validation target for the fuzz infrastructure itself.

use crate::{
    CaseGenerator, CaseResult, Corpus, CorpusEntry, Engine, FailureArtifact, FuzzCase, FuzzTarget,
    MutationKind, RunConfig,
};
use std::path::PathBuf;

pub const NAME: &str = "smoke";

#[derive(Clone, Copy, Debug, Default)]
pub struct SmokeTarget;

impl FuzzTarget for SmokeTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            CaseResult::invariant_failure("smoke target received a case for another target")
        } else {
            CaseResult::safe_reject("smoke infrastructure case consumed")
        }
    }
}

/// Run small in-memory checks before the configured smoke campaign. These
/// checks exercise generator determinism, bounds, corpus mutation, summaries,
/// serialization, and exact artifact-to-case reconstruction.
pub fn validate_infrastructure() -> Result<(), String> {
    let empty = Corpus::empty();
    let left = generated_cases(100, 16, empty.clone(), 16)?;
    let right = generated_cases(100, 16, empty.clone(), 16)?;
    if left != right {
        return Err("same seed produced a different generated sequence".to_string());
    }
    let other = generated_cases(101, 16, empty.clone(), 16)?;
    if left
        .iter()
        .zip(&other)
        .all(|(left, right)| left.input == right.input)
    {
        return Err("different seeds produced identical input sequences".to_string());
    }
    if left.iter().any(|case| case.input_len() > 16) {
        return Err("generator exceeded max_len".to_string());
    }

    let corpus = Corpus::from_entries(
        vec![
            CorpusEntry::new("empty.bin", Vec::new()),
            CorpusEntry::new("small.bin", vec![1, 2, 3]),
        ],
        16,
    )
    .map_err(|error| error.to_string())?;
    let corpus_cases = generated_cases(100, 16, corpus, 16)?;
    if !corpus_cases.iter().any(|case| case.corpus_origin.is_some()) {
        return Err("small corpus did not produce a corpus mutation".to_string());
    }

    let sample = FuzzCase {
        target: NAME.to_string(),
        seed: 100,
        iteration: 7,
        input: vec![0, 1, 0xfe, 0xff],
        mutation: MutationKind::CorpusXor,
        corpus_origin: Some("small.bin".to_string()),
    };
    let artifact = FailureArtifact::from_case(&sample, "smoke serialization check");
    let decoded =
        FailureArtifact::deserialize(&artifact.serialize()).map_err(|error| error.to_string())?;
    if decoded.to_case() != sample {
        return Err("serialized failure did not reconstruct the exact case".to_string());
    }

    let zero = summary(0)?;
    if zero.cases_generated != 0 || zero.cases_completed != 0 {
        return Err("zero iterations did not execute exactly zero cases".to_string());
    }
    let one = summary(1)?;
    if one.cases_generated != 1 || one.cases_completed != 1 {
        return Err("one iteration did not execute exactly one case".to_string());
    }
    if summary(16)?.render() != summary(16)?.render() {
        return Err("identical runs produced different summaries".to_string());
    }
    if summary_with_seed(100, 16)?.generated_digest()
        == summary_with_seed(101, 16)?.generated_digest()
    {
        return Err("different seeds produced identical generated digests".to_string());
    }
    Ok(())
}

fn generated_cases(
    seed: u64,
    max_len: usize,
    corpus: Corpus,
    count: u64,
) -> Result<Vec<FuzzCase>, String> {
    let mut generator = CaseGenerator::new(NAME, seed, max_len, corpus)?;
    Ok((0..count)
        .map(|iteration| generator.case(iteration))
        .collect())
}

fn summary(iterations: u64) -> Result<crate::RunSummary, String> {
    summary_with_seed(100, iterations)
}

fn summary_with_seed(seed: u64, iterations: u64) -> Result<crate::RunSummary, String> {
    let config = RunConfig {
        target: NAME.to_string(),
        seed,
        iterations,
        max_len: 16,
        failure_dir: PathBuf::from("fuzz_failures"),
    };
    Engine
        .run(&config, Corpus::empty(), &mut SmokeTarget)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_infrastructure;

    #[test]
    fn smoke_self_validation_passes() {
        validate_infrastructure().expect("smoke infrastructure validation");
    }
}
