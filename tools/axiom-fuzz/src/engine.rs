//! Reusable bounded execution and replay engine.

use crate::failure::FailureArtifact;
use crate::limits::{MAX_INPUT_LEN, MAX_ITERATIONS};
use crate::{CaseGenerator, CaseResult, Corpus, FuzzCase, ResultClass, RunSummary};
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunConfig {
    pub target: String,
    pub seed: u64,
    pub iterations: u64,
    pub max_len: usize,
    pub failure_dir: PathBuf,
}

impl RunConfig {
    pub fn validate(&self) -> io::Result<()> {
        if self.target.is_empty()
            || self.target.len() > 64
            || !self
                .target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(invalid_input(
                "fuzz target must be 1..=64 ASCII letters, digits, '-' or '_'",
            ));
        }
        if self.iterations > MAX_ITERATIONS {
            return Err(invalid_input(format!(
                "iterations {} exceeds hard limit {MAX_ITERATIONS}",
                self.iterations
            )));
        }
        if self.max_len > MAX_INPUT_LEN {
            return Err(invalid_input(format!(
                "max_len {} exceeds hard limit {MAX_INPUT_LEN}",
                self.max_len
            )));
        }
        Ok(())
    }
}

/// Programmatic contract implemented by present and future fuzz targets.
pub trait FuzzTarget {
    fn name(&self) -> &'static str;
    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Engine;

impl Engine {
    pub fn run<T: FuzzTarget>(
        &self,
        config: &RunConfig,
        corpus: Corpus,
        target: &mut T,
    ) -> io::Result<RunSummary> {
        config.validate()?;
        ensure_target_matches(&config.target, target.name())?;

        let mut generator = CaseGenerator::new(&config.target, config.seed, config.max_len, corpus)
            .map_err(invalid_input)?;
        let mut summary = RunSummary::new(
            &config.target,
            config.seed,
            config.iterations,
            config.max_len,
        );

        for iteration in 0..config.iterations {
            let case = generator.case(iteration);
            summary.record_generated(&case);
            let result = if case.input_len() > config.max_len {
                CaseResult::invariant_failure(format!(
                    "generator emitted {} bytes above max_len {}",
                    case.input_len(),
                    config.max_len
                ))
            } else {
                target.evaluate(&case)
            };
            summary.record_result(&result);

            if result.class == ResultClass::KernelInvariantFailure {
                FailureArtifact::from_case(&case, &result.reason)
                    .write_to_dir(&config.failure_dir)?;
                break;
            }
        }

        Ok(summary)
    }

    /// Evaluate exactly the bytes stored in an artifact. No case generation is
    /// performed during replay.
    pub fn replay<T: FuzzTarget>(
        &self,
        artifact: &FailureArtifact,
        target: &mut T,
    ) -> io::Result<RunSummary> {
        ensure_target_matches(&artifact.target, target.name())?;
        if artifact.input.len() > MAX_INPUT_LEN {
            return Err(invalid_input(format!(
                "replay input is {} bytes; hard limit is {MAX_INPUT_LEN}",
                artifact.input.len()
            )));
        }

        let case = artifact.to_case();
        let mut summary = RunSummary::new(&artifact.target, artifact.seed, 1, case.input_len());
        summary.record_generated(&case);
        let result = target.evaluate(&case);
        summary.record_result(&result);
        Ok(summary)
    }
}

fn ensure_target_matches(configured: &str, implemented: &str) -> io::Result<()> {
    if configured == implemented {
        Ok(())
    } else {
        Err(invalid_input(format!(
            "configured target {configured:?} does not match implementation {implemented:?}"
        )))
    }
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::{Engine, FuzzTarget, RunConfig};
    use crate::{CaseResult, Corpus, FailureArtifact, FuzzCase, MutationKind, ResultClass};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct SafeTarget;

    impl FuzzTarget for SafeTarget {
        fn name(&self) -> &'static str {
            "smoke"
        }

        fn evaluate(&mut self, _case: &FuzzCase) -> CaseResult {
            CaseResult::safe_reject("consumed")
        }
    }

    struct FailingTarget;

    impl FuzzTarget for FailingTarget {
        fn name(&self) -> &'static str {
            "smoke"
        }

        fn evaluate(&mut self, _case: &FuzzCase) -> CaseResult {
            CaseResult::invariant_failure("injected invariant failure")
        }
    }

    #[derive(Default)]
    struct RecordingTarget {
        seen: Vec<FuzzCase>,
    }

    impl FuzzTarget for RecordingTarget {
        fn name(&self) -> &'static str {
            "smoke"
        }

        fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
            self.seen.push(case.clone());
            CaseResult::safe_reject("recorded")
        }
    }

    fn failure_dir(label: &str) -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "axiom-fuzz-engine-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn config(iterations: u64, failure_dir: PathBuf) -> RunConfig {
        RunConfig {
            target: "smoke".to_string(),
            seed: 20260903,
            iterations,
            max_len: 32,
            failure_dir,
        }
    }

    #[test]
    fn zero_iterations_runs_exactly_zero_cases() {
        let mut target = SafeTarget;
        let summary = Engine
            .run(
                &config(0, failure_dir("zero")),
                Corpus::empty(),
                &mut target,
            )
            .expect("zero-iteration run");
        assert_eq!(summary.cases_generated, 0);
        assert_eq!(summary.cases_completed, 0);
    }

    #[test]
    fn one_iteration_runs_exactly_one_case() {
        let mut target = SafeTarget;
        let summary = Engine
            .run(&config(1, failure_dir("one")), Corpus::empty(), &mut target)
            .expect("one-iteration run");
        assert_eq!(summary.cases_generated, 1);
        assert_eq!(summary.cases_completed, 1);
        assert_eq!(summary.safe_rejects, 1);
    }

    #[test]
    fn all_requested_iterations_complete_without_failure() {
        let mut target = SafeTarget;
        let summary = Engine
            .run(
                &config(257, failure_dir("exact")),
                Corpus::empty(),
                &mut target,
            )
            .expect("bounded run");
        assert_eq!(summary.cases_generated, 257);
        assert_eq!(summary.cases_completed, 257);
    }

    #[test]
    fn invariant_failure_writes_artifact_and_returns_failure_status() {
        let directory = failure_dir("invariant");
        let mut target = FailingTarget;
        let summary = Engine
            .run(&config(10, directory.clone()), Corpus::empty(), &mut target)
            .expect("failing run");
        assert_eq!(summary.invariant_failures, 1);
        assert_eq!(summary.exit_code(), 1);
        assert_eq!(summary.cases_generated, 1);

        let artifact_path = directory.join("smoke/seed-20260903-iteration-0.txt");
        let artifact = FailureArtifact::read(&artifact_path).expect("load written artifact");
        assert_eq!(artifact.seed, 20260903);
        assert_eq!(artifact.iteration, 0);
        assert_eq!(artifact.input, Vec::<u8>::new());
        assert_eq!(artifact.mutation, MutationKind::Empty);
        assert_eq!(artifact.reason, "injected invariant failure");
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn replay_uses_exact_stored_bytes_and_metadata() {
        let expected = FuzzCase {
            target: "smoke".to_string(),
            seed: 81,
            iteration: 999,
            input: vec![0, 0xff, 4, 7],
            mutation: MutationKind::CorpusXor,
            corpus_origin: Some("seed.bin".to_string()),
        };
        let artifact = FailureArtifact::from_case(&expected, "test failure");
        let mut target = RecordingTarget::default();
        let summary = Engine
            .replay(&artifact, &mut target)
            .expect("replay artifact");
        assert_eq!(target.seen, [expected]);
        assert_eq!(summary.cases_generated, 1);
        assert_eq!(summary.cases_completed, 1);
        assert_eq!(summary.invariant_failures, 0);
    }

    #[test]
    fn result_class_names_remain_aligned_with_robustness_model() {
        assert_eq!(ResultClass::SafeReject.as_str(), "SAFE_REJECT");
        assert_eq!(
            ResultClass::ContainedUserFault.as_str(),
            "CONTAINED_USER_FAULT"
        );
        assert_eq!(
            ResultClass::BoundedResourceExhaustion.as_str(),
            "BOUNDED_RESOURCE_EXHAUSTION"
        );
        assert_eq!(
            ResultClass::KernelInvariantFailure.as_str(),
            "KERNEL_INVARIANT_FAILURE"
        );
    }
}
