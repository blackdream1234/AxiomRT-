//! Stable representation of one generated fuzz case.

use std::fmt;
use std::str::FromStr;

/// The deterministic operation used to construct an input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationKind {
    Empty,
    BoundaryMinimum,
    BoundaryMaximum,
    RandomBytes,
    BitFlip,
    ByteInsert,
    CorpusXor,
    CorpusTruncate,
}

impl MutationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::BoundaryMinimum => "boundary-minimum",
            Self::BoundaryMaximum => "boundary-maximum",
            Self::RandomBytes => "random-bytes",
            Self::BitFlip => "bit-flip",
            Self::ByteInsert => "byte-insert",
            Self::CorpusXor => "corpus-xor",
            Self::CorpusTruncate => "corpus-truncate",
        }
    }
}

impl fmt::Display for MutationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MutationKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "empty" => Ok(Self::Empty),
            "boundary-minimum" => Ok(Self::BoundaryMinimum),
            "boundary-maximum" => Ok(Self::BoundaryMaximum),
            "random-bytes" => Ok(Self::RandomBytes),
            "bit-flip" => Ok(Self::BitFlip),
            "byte-insert" => Ok(Self::ByteInsert),
            "corpus-xor" => Ok(Self::CorpusXor),
            "corpus-truncate" => Ok(Self::CorpusTruncate),
            _ => Err(format!("unknown mutation kind: {value}")),
        }
    }
}

/// One replayable fuzz input and its generation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuzzCase {
    pub target: String,
    pub seed: u64,
    pub iteration: u64,
    pub input: Vec<u8>,
    pub mutation: MutationKind,
    pub corpus_origin: Option<String>,
}

impl FuzzCase {
    pub fn input_len(&self) -> usize {
        self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::MutationKind;
    use std::str::FromStr;

    #[test]
    fn mutation_names_round_trip() {
        for mutation in [
            MutationKind::Empty,
            MutationKind::BoundaryMinimum,
            MutationKind::BoundaryMaximum,
            MutationKind::RandomBytes,
            MutationKind::BitFlip,
            MutationKind::ByteInsert,
            MutationKind::CorpusXor,
            MutationKind::CorpusTruncate,
        ] {
            assert_eq!(MutationKind::from_str(mutation.as_str()), Ok(mutation));
        }
    }
}
