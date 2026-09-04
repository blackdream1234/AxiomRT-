//! Result classes and deterministic run summaries.

use crate::FuzzCase;
use std::fmt::Write;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Robustness outcomes defined by docs/36_ROBUSTNESS_AND_FUZZING.md.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultClass {
    SafeReject,
    ContainedUserFault,
    BoundedResourceExhaustion,
    KernelInvariantFailure,
}

impl ResultClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SafeReject => "SAFE_REJECT",
            Self::ContainedUserFault => "CONTAINED_USER_FAULT",
            Self::BoundedResourceExhaustion => "BOUNDED_RESOURCE_EXHAUSTION",
            Self::KernelInvariantFailure => "KERNEL_INVARIANT_FAILURE",
        }
    }
}

/// A target's classification and stable diagnostic for one input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseResult {
    pub class: ResultClass,
    pub reason: String,
}

impl CaseResult {
    pub fn new(class: ResultClass, reason: impl Into<String>) -> Self {
        Self {
            class,
            reason: reason.into(),
        }
    }

    pub fn safe_reject(reason: impl Into<String>) -> Self {
        Self::new(ResultClass::SafeReject, reason)
    }

    pub fn invariant_failure(reason: impl Into<String>) -> Self {
        Self::new(ResultClass::KernelInvariantFailure, reason)
    }
}

/// Aggregate counters and digest for one bounded run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSummary {
    pub target: String,
    pub seed: u64,
    pub iterations: u64,
    pub max_len: usize,
    pub cases_generated: u64,
    pub cases_completed: u64,
    pub safe_rejects: u64,
    pub contained_faults: u64,
    pub resource_exhaustions: u64,
    pub invariant_failures: u64,
    generated_digest: u64,
}

impl RunSummary {
    pub fn new(target: impl Into<String>, seed: u64, iterations: u64, max_len: usize) -> Self {
        let target = target.into();
        let mut summary = Self {
            target,
            seed,
            iterations,
            max_len,
            cases_generated: 0,
            cases_completed: 0,
            safe_rejects: 0,
            contained_faults: 0,
            resource_exhaustions: 0,
            invariant_failures: 0,
            generated_digest: FNV_OFFSET_BASIS,
        };
        let target_bytes = summary.target.clone();
        summary.digest_bytes(target_bytes.as_bytes());
        summary.digest_bytes(&seed.to_le_bytes());
        summary.digest_bytes(&iterations.to_le_bytes());
        summary.digest_bytes(&(max_len as u64).to_le_bytes());
        summary
    }

    pub fn record_generated(&mut self, case: &FuzzCase) {
        self.cases_generated += 1;
        self.digest_bytes(case.target.as_bytes());
        self.digest_bytes(&case.seed.to_le_bytes());
        self.digest_bytes(&case.iteration.to_le_bytes());
        self.digest_bytes(&(case.input.len() as u64).to_le_bytes());
        self.digest_bytes(case.mutation.as_str().as_bytes());
        match &case.corpus_origin {
            Some(origin) => {
                self.digest_bytes(&[1]);
                self.digest_bytes(origin.as_bytes());
            }
            None => self.digest_bytes(&[0]),
        }
        self.digest_bytes(&case.input);
    }

    pub fn record_result(&mut self, result: &CaseResult) {
        self.cases_completed += 1;
        match result.class {
            ResultClass::SafeReject => self.safe_rejects += 1,
            ResultClass::ContainedUserFault => self.contained_faults += 1,
            ResultClass::BoundedResourceExhaustion => self.resource_exhaustions += 1,
            ResultClass::KernelInvariantFailure => self.invariant_failures += 1,
        }
    }

    pub const fn generated_digest(&self) -> u64 {
        self.generated_digest
    }

    pub const fn passed(&self) -> bool {
        self.invariant_failures == 0
    }

    pub const fn exit_code(&self) -> u8 {
        if self.passed() {
            0
        } else {
            1
        }
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        writeln!(output, "FUZZ target={}", self.target).expect("writing to String cannot fail");
        writeln!(output, "seed={}", self.seed).expect("writing to String cannot fail");
        writeln!(output, "iterations={}", self.iterations).expect("writing to String cannot fail");
        writeln!(output, "max_len={}", self.max_len).expect("writing to String cannot fail");
        writeln!(output, "cases_generated={}", self.cases_generated)
            .expect("writing to String cannot fail");
        writeln!(output, "cases_completed={}", self.cases_completed)
            .expect("writing to String cannot fail");
        writeln!(output, "safe_rejects={}", self.safe_rejects)
            .expect("writing to String cannot fail");
        writeln!(output, "contained_faults={}", self.contained_faults)
            .expect("writing to String cannot fail");
        writeln!(output, "resource_exhaustions={}", self.resource_exhaustions)
            .expect("writing to String cannot fail");
        writeln!(output, "invariant_failures={}", self.invariant_failures)
            .expect("writing to String cannot fail");
        writeln!(output, "generated_digest={:016x}", self.generated_digest)
            .expect("writing to String cannot fail");
        writeln!(
            output,
            "FUZZ RESULT: {}",
            if self.passed() { "PASS" } else { "FAIL" }
        )
        .expect("writing to String cannot fail");
        output
    }

    fn digest_bytes(&mut self, bytes: &[u8]) {
        self.generated_digest ^= bytes.len() as u64;
        self.generated_digest = self.generated_digest.wrapping_mul(FNV_PRIME);
        for byte in bytes {
            self.generated_digest ^= u64::from(*byte);
            self.generated_digest = self.generated_digest.wrapping_mul(FNV_PRIME);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseResult, ResultClass, RunSummary};
    use crate::{FuzzCase, MutationKind};

    fn case() -> FuzzCase {
        FuzzCase {
            target: "smoke".to_string(),
            seed: 5,
            iteration: 0,
            input: vec![1, 2, 3],
            mutation: MutationKind::RandomBytes,
            corpus_origin: None,
        }
    }

    #[test]
    fn deterministic_summary_counters_and_digest() {
        let mut left = RunSummary::new("smoke", 5, 2, 8);
        let mut right = RunSummary::new("smoke", 5, 2, 8);
        for summary in [&mut left, &mut right] {
            summary.record_generated(&case());
            summary.record_result(&CaseResult::safe_reject("expected"));
            summary.record_generated(&case());
            summary.record_result(&CaseResult::new(
                ResultClass::BoundedResourceExhaustion,
                "bounded",
            ));
        }
        assert_eq!(left, right);
        assert_eq!(left.cases_generated, 2);
        assert_eq!(left.cases_completed, 2);
        assert_eq!(left.safe_rejects, 1);
        assert_eq!(left.resource_exhaustions, 1);
        assert!(left.render().ends_with("FUZZ RESULT: PASS\n"));
    }

    #[test]
    fn invariant_failure_has_nonzero_exit_logic() {
        let mut summary = RunSummary::new("smoke", 5, 1, 8);
        summary.record_result(&CaseResult::invariant_failure("test"));
        assert_eq!(summary.exit_code(), 1);
        assert!(summary.render().ends_with("FUZZ RESULT: FAIL\n"));
    }
}
