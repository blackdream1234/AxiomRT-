//! Stable, human-readable invariant-failure artifacts and exact replay input.

use crate::limits::MAX_INPUT_LEN;
use crate::{FuzzCase, MutationKind};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::{FromStr, Lines};

const HEADER: &str = "AXIOM_FUZZ_FAILURE_V1";
const MAX_ARTIFACT_BYTES: u64 = (MAX_INPUT_LEN as u64 * 2) + 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureArtifact {
    pub target: String,
    pub seed: u64,
    pub iteration: u64,
    pub input: Vec<u8>,
    pub mutation: MutationKind,
    pub corpus_origin: Option<String>,
    pub reason: String,
}

impl FailureArtifact {
    pub fn from_case(case: &FuzzCase, reason: impl Into<String>) -> Self {
        Self {
            target: case.target.clone(),
            seed: case.seed,
            iteration: case.iteration,
            input: case.input.clone(),
            mutation: case.mutation,
            corpus_origin: case.corpus_origin.clone(),
            reason: reason.into(),
        }
    }

    pub fn to_case(&self) -> FuzzCase {
        FuzzCase {
            target: self.target.clone(),
            seed: self.seed,
            iteration: self.iteration,
            input: self.input.clone(),
            mutation: self.mutation,
            corpus_origin: self.corpus_origin.clone(),
        }
    }

    pub fn serialize(&self) -> String {
        let origin_present = u8::from(self.corpus_origin.is_some());
        let origin = self.corpus_origin.as_deref().unwrap_or("");
        format!(
            "{HEADER}\n\
             classification=KERNEL_INVARIANT_FAILURE\n\
             target={}\n\
             seed={}\n\
             iteration={}\n\
             input_hex={}\n\
             input_len={}\n\
             mutation={}\n\
             corpus_origin_present={}\n\
             corpus_origin={}\n\
             reason={}\n",
            escape_text(&self.target),
            self.seed,
            self.iteration,
            encode_hex(&self.input),
            self.input.len(),
            self.mutation,
            origin_present,
            escape_text(origin),
            escape_text(&self.reason)
        )
    }

    pub fn deserialize(text: &str) -> io::Result<Self> {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err(invalid_data("unsupported failure artifact header"));
        }
        if next_field(&mut lines, "classification")? != "KERNEL_INVARIANT_FAILURE" {
            return Err(invalid_data(
                "failure artifact classification is not KERNEL_INVARIANT_FAILURE",
            ));
        }

        let target = unescape_text(next_field(&mut lines, "target")?)?;
        let seed = parse_number(next_field(&mut lines, "seed")?, "seed")?;
        let iteration = parse_number(next_field(&mut lines, "iteration")?, "iteration")?;
        let input = decode_hex(next_field(&mut lines, "input_hex")?)?;
        let input_len: usize = parse_number(next_field(&mut lines, "input_len")?, "input_len")?;
        if input.len() != input_len {
            return Err(invalid_data(format!(
                "input_len is {input_len}, but input_hex contains {} bytes",
                input.len()
            )));
        }
        let mutation =
            MutationKind::from_str(next_field(&mut lines, "mutation")?).map_err(invalid_data)?;
        let origin_present = match next_field(&mut lines, "corpus_origin_present")? {
            "0" => false,
            "1" => true,
            value => {
                return Err(invalid_data(format!(
                    "corpus_origin_present must be 0 or 1, got {value}"
                )))
            }
        };
        let origin = unescape_text(next_field(&mut lines, "corpus_origin")?)?;
        let reason = unescape_text(next_field(&mut lines, "reason")?)?;
        if lines.next().is_some() {
            return Err(invalid_data("unexpected trailing failure artifact fields"));
        }

        Ok(Self {
            target,
            seed,
            iteration,
            input,
            mutation,
            corpus_origin: origin_present.then_some(origin),
            reason,
        })
    }

    pub fn write_to_dir(&self, root: &Path) -> io::Result<PathBuf> {
        let directory = root.join(sanitize_target(&self.target));
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!(
            "seed-{}-iteration-{}.txt",
            self.seed, self.iteration
        ));
        fs::write(&path, self.serialize())?;
        Ok(path)
    }

    pub fn read(path: &Path) -> io::Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(MAX_ARTIFACT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
            return Err(invalid_data(format!(
                "failure artifact exceeds hard limit {MAX_ARTIFACT_BYTES}"
            )));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| invalid_data("failure artifact is not valid UTF-8"))?;
        Self::deserialize(&text)
    }
}

fn next_field<'a>(lines: &mut Lines<'a>, expected: &str) -> io::Result<&'a str> {
    let line = lines
        .next()
        .ok_or_else(|| invalid_data(format!("missing field {expected}")))?;
    let (name, value) = line
        .split_once('=')
        .ok_or_else(|| invalid_data(format!("malformed field {expected}")))?;
    if name != expected {
        return Err(invalid_data(format!(
            "expected field {expected}, found {name}"
        )));
    }
    Ok(value)
}

fn parse_number<T>(value: &str, field: &str) -> io::Result<T>
where
    T: FromStr,
{
    value
        .parse()
        .map_err(|_| invalid_data(format!("invalid numeric field {field}: {value}")))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> io::Result<Vec<u8>> {
    if value.len() > MAX_INPUT_LEN * 2 {
        return Err(invalid_data(format!(
            "input_hex exceeds maximum encoded length {}",
            MAX_INPUT_LEN * 2
        )));
    }
    if !value.len().is_multiple_of(2) {
        return Err(invalid_data("input_hex has odd length"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_digit(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid_data("input_hex contains a non-lowercase-hex digit")),
    }
}

fn escape_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn unescape_text(value: &str) -> io::Result<String> {
    let mut unescaped = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            unescaped.push(character);
            continue;
        }
        match characters.next() {
            Some('\\') => unescaped.push('\\'),
            Some('n') => unescaped.push('\n'),
            Some('r') => unescaped.push('\r'),
            Some('t') => unescaped.push('\t'),
            Some(other) => return Err(invalid_data(format!("unknown escape sequence \\{other}"))),
            None => return Err(invalid_data("trailing escape character")),
        }
    }
    Ok(unescaped)
}

fn sanitize_target(target: &str) -> String {
    target
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::FailureArtifact;
    use crate::{FuzzCase, MutationKind};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn artifact() -> FailureArtifact {
        FailureArtifact::from_case(
            &FuzzCase {
                target: "smoke".to_string(),
                seed: 123,
                iteration: 7,
                input: vec![0, 1, 0xfe, 0xff],
                mutation: MutationKind::CorpusXor,
                corpus_origin: Some("line\\nbreak.bin".to_string()),
            },
            "invariant\nreason",
        )
    }

    fn temp_dir() -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "axiom-fuzz-failure-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        path
    }

    #[test]
    fn failure_serialization_round_trips() {
        let expected = artifact();
        let actual = FailureArtifact::deserialize(&expected.serialize()).expect("deserialize");
        assert_eq!(actual, expected);
        assert_eq!(actual.to_case().input, [0, 1, 0xfe, 0xff]);
    }

    #[test]
    fn failure_file_reloads_exact_bytes_from_deterministic_path() {
        let directory = temp_dir();
        let expected = artifact();
        let path = expected.write_to_dir(&directory).expect("write artifact");
        assert!(path.ends_with("smoke/seed-123-iteration-7.txt"));
        let actual = FailureArtifact::read(&path).expect("read artifact");
        assert_eq!(actual, expected);
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
