//! Deterministic adversarial target for the storage_service protocol.
//!
//! Models the docs/29 §4 bounded block protocol exactly as the U-mode
//! storage_service implements it (os_boot.rs storage_body), with one
//! resolved contract decision from AXIOM-ROBUST-006: decimal numbers
//! that overflow u64 are rejected as `ERR malformed`. The pre-006B
//! runtime parsed them with wrapping arithmetic, so `READ block=` 2^64
//! aliased block 0 — a real wrong-answer defect fixed by
//! AXIOM-ROBUST-006B (docs/29 §4, docs/36 §5.8).
//!
//! The model is checked differentially: an independent std-based
//! grammar oracle re-derives the expected outcome for every request
//! and any divergence, wrong content, out-of-bound reply, or panic is
//! a KERNEL_INVARIANT_FAILURE. This is host-model evidence for the
//! protocol; the live U-mode service and its IPC transport remain
//! covered by the storage QEMU test.

use crate::{CaseResult, FuzzCase, FuzzTarget};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NAME: &str = "storage";
pub const MAX_STORAGE_OPS_PER_CASE: usize = 16;

/// Transport bound (docs/29 §4): ≤ 64-byte request and reply. Every
/// in-tree client assembles requests in a 64-byte buffer clamped at 63
/// bytes, so the service never receives more.
pub const REQ_MAX: usize = 64;
pub const REPLY_MAX: usize = 64;
pub const BLOCK_SIZE: usize = 48;
pub const NUM_BLOCKS: u64 = 8;

/// The read-only block image (docs/29 §8, mirrored verbatim from the
/// os_boot.rs statics; blocks 4-6 are the docs/32 app records).
pub const BLOCKS: [&[u8]; 8] = [
    b"AXSTOR v1 blocks=8 bs=48 ro=1",
    b"AxiomRT v1.6-storage-backed-loader eval stage",
    b"AxiomRT microkernel safety runtime",
    b"apps: hello counter fault_demo prio=2",
    b"AXAPP1 hello 0 4096 4096 1 console 8192 0a6d",
    b"AXAPP1 counter 0 4096 4096 1 console 8192 0b59",
    b"AXAPP1 fault_demo 0 4096 4096 1 none 8192 0b36",
    b"reserved",
];

const P_INFO: &[u8] = b"INFO";
const P_READ: &[u8] = b"READ block=";
const P_RANGE: &[u8] = b"READ_RANGE start=";
const P_COUNT: &[u8] = b" count=";
pub const R_INFO: &[u8] = b"OK block_size=48 blocks=8 readonly=true";
pub const R_DATA: &[u8] = b"OK data=";
pub const E_BLOCK: &[u8] = b"ERR bad_block";
pub const E_MANY: &[u8] = b"ERR too_many_blocks";
pub const E_MAL: &[u8] = b"ERR malformed";

/// One protocol interaction outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Request exceeds the 64-byte transport bound: the bounded IPC
    /// path rejects it before the service parses anything.
    TransportReject,
    /// Zero-length message: `storage_body` tests `if r <= 0 { continue }`
    /// and loops without answering, so the service produces **no reply
    /// at all** — not an error reply. No in-tree client sends one (every
    /// sender writes at least an opcode prefix), but a client that did
    /// would wait for a reply that never comes.
    Ignored,
    Reply(Vec<u8>),
}

/// Maximum decimal digits the service accepts (AXIOM-ROBUST-006B):
/// nineteen nines is below `u64::MAX`, so a number of at most this many
/// digits can never overflow. The runtime bounds the digit *count*
/// rather than comparing against a 64-bit constant, because such a
/// constant is materialised out of kernel `.rodata` and U-mode must
/// never reference it (docs/25 §2).
pub const DEC_MAX_DIGITS: usize = 19;

/// Decimal parse mirroring the fixed parse_dec_stop contract: scan
/// digits from `from`; return (None, end) when no digit is present or
/// more than `DEC_MAX_DIGITS` digits are present. Every None case
/// answers `ERR malformed`. Because the bound is on digit count, an
/// over-long number is rejected even when leading zeros would make its
/// value small — the documented consequence of keeping the check free
/// of wide constants. The digit scan always completes so trailing-junk
/// detection stays position-exact.
pub fn parse_dec_checked(bytes: &[u8], from: usize) -> (Option<u64>, usize) {
    let mut value: u64 = 0;
    let mut index = from;
    let mut digits = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            break;
        }
        if digits < DEC_MAX_DIGITS {
            value = value * 10 + u64::from(byte - b'0');
        }
        digits += 1;
        index += 1;
    }
    if digits > 0 && digits <= DEC_MAX_DIGITS {
        (Some(value), index)
    } else {
        (None, index)
    }
}

/// The storage_service protocol model (docs/29 §4; byte-order mirror
/// of storage_body with the checked-parse contract).
pub fn storage_reply(request: &[u8]) -> Outcome {
    if request.len() > REQ_MAX {
        return Outcome::TransportReject;
    }
    if request.is_empty() {
        return Outcome::Ignored;
    }
    if request == P_INFO {
        return Outcome::Reply(R_INFO.to_vec());
    }
    if let Some(rest_at) = strip_prefix_at(request, P_READ) {
        let (block, end) = parse_dec_checked(request, rest_at);
        return match block {
            Some(block) if end == request.len() => Outcome::Reply(block_reply(block)),
            _ => Outcome::Reply(E_MAL.to_vec()),
        };
    }
    if let Some(rest_at) = strip_prefix_at(request, P_RANGE) {
        let (start, i) = parse_dec_checked(request, rest_at);
        let after = i + P_COUNT.len();
        let separator_ok = after <= request.len() && &request[i..after] == P_COUNT;
        let Some(start) = start else {
            return Outcome::Reply(E_MAL.to_vec());
        };
        if !separator_ok {
            return Outcome::Reply(E_MAL.to_vec());
        }
        let (count, end) = parse_dec_checked(request, after);
        return match count {
            Some(count) if end == request.len() => {
                if count != 1 {
                    Outcome::Reply(E_MANY.to_vec())
                } else {
                    Outcome::Reply(block_reply(start))
                }
            }
            _ => Outcome::Reply(E_MAL.to_vec()),
        };
    }
    Outcome::Reply(E_MAL.to_vec())
}

fn block_reply(block: u64) -> Vec<u8> {
    if block < NUM_BLOCKS {
        let mut reply = R_DATA.to_vec();
        reply.extend_from_slice(BLOCKS[block as usize]);
        reply
    } else {
        E_BLOCK.to_vec()
    }
}

fn strip_prefix_at(request: &[u8], prefix: &[u8]) -> Option<usize> {
    if request.len() >= prefix.len() && &request[..prefix.len()] == prefix {
        Some(prefix.len())
    } else {
        None
    }
}

// ---- Independent grammar oracle -----------------------------------------

/// What the docs/29 §4 contract requires for a request, derived by an
/// independent std-string implementation (not the byte-mirror model).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expected {
    Transport,
    /// Zero-length message: consumed and ignored, no reply.
    Ignored,
    Info,
    /// Well-formed in-range read: the reply must be exactly this block.
    Data(usize),
    BadBlock,
    TooMany,
    Malformed,
}

pub fn expected_outcome(request: &[u8]) -> Expected {
    if request.len() > REQ_MAX {
        return Expected::Transport;
    }
    if request.is_empty() {
        return Expected::Ignored;
    }
    let Ok(text) = std::str::from_utf8(request) else {
        // Every request the grammar accepts is pure ASCII (`INFO`, or a
        // prefix followed only by digits and the ` count=` separator,
        // ending exactly at the request end). A non-UTF-8 request can
        // therefore never be accepted: it either fails the exact match,
        // the prefix, or the trailing-junk rule. So it is malformed.
        return Expected::Malformed;
    };
    if text == "INFO" {
        return Expected::Info;
    }
    if let Some(number) = text.strip_prefix("READ block=") {
        return match classify_number(number) {
            Number::Value(block) if block < NUM_BLOCKS => Expected::Data(block as usize),
            Number::Value(_) => Expected::BadBlock,
            _ => Expected::Malformed,
        };
    }
    if let Some(rest) = text.strip_prefix("READ_RANGE start=") {
        // Grammar: <digits> " count=" <digits>, nothing else.
        let digit_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let (start_digits, tail) = rest.split_at(digit_end);
        if start_digits.is_empty() || classify_number(start_digits) == Number::TooLong {
            return Expected::Malformed;
        }
        let Some(count_text) = tail.strip_prefix(" count=") else {
            return Expected::Malformed;
        };
        return match classify_number(count_text) {
            Number::Value(1) => match classify_number(start_digits) {
                Number::Value(start) if start < NUM_BLOCKS => Expected::Data(start as usize),
                _ => Expected::BadBlock,
            },
            Number::Value(_) => Expected::TooMany,
            _ => Expected::Malformed,
        };
    }
    Expected::Malformed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Number {
    Value(u64),
    TooLong,
    Malformed,
}

/// Full-token decimal classification, derived independently of the
/// byte-mirror model: all-digits and at most `DEC_MAX_DIGITS` long →
/// Value; all-digits but longer → TooLong (rejected regardless of the
/// numeric value, per the digit-count contract); anything else (empty,
/// sign, junk, trailing bytes) → Malformed.
fn classify_number(token: &str) -> Number {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Number::Malformed;
    }
    if token.len() > DEC_MAX_DIGITS {
        return Number::TooLong;
    }
    match token.parse::<u64>() {
        Ok(value) => Number::Value(value),
        // Unreachable: <= 19 digits always fits a u64. Mapped
        // defensively rather than panicking.
        Err(_) => Number::TooLong,
    }
}

// ---- Fixed boundary bank -------------------------------------------------

pub const FIXED_SCENARIO_NAMES: [&str; 30] = [
    "info_exact",
    "info_trailing_junk",
    "read_block_0",
    "read_block_7",
    "read_block_8",
    "read_block_leading_zeros",
    "read_block_u64_max",
    "read_block_overflow_2p64",
    "read_block_overflow_26_digits",
    "read_block_negative",
    "read_block_no_digits",
    "read_block_space_before_digit",
    "read_block_trailing_junk",
    "read_lowercase",
    "range_count1_first",
    "range_count1_last",
    "range_count1_beyond",
    "range_count0",
    "range_count2",
    "range_count_overflow",
    "range_start_overflow",
    "range_missing_count",
    "range_bad_separator",
    "range_trailing_junk",
    "empty_request_ignored",
    "malformed_whitespace",
    "malformed_nul",
    "malformed_binary",
    "request_len_64_exact",
    "transport_oversize",
];

const MANDATORY_SCENARIOS: u64 = FIXED_SCENARIO_NAMES.len() as u64;

fn mandatory_scenario(index: u64) -> Vec<Vec<u8>> {
    let req = |bytes: &[u8]| vec![bytes.to_vec()];
    match index {
        0 => req(b"INFO"),
        1 => req(b"INFO "),
        2 => req(b"READ block=0"),
        3 => req(b"READ block=7"),
        4 => req(b"READ block=8"),
        5 => req(b"READ block=0000007"),
        6 => req(b"READ block=18446744073709551615"),
        // The AXIOM-ROBUST-006 regression: 2^64 must never alias block 0.
        7 => req(b"READ block=18446744073709551616"),
        8 => req(b"READ block=99999999999999999999999999"),
        9 => req(b"READ block=-1"),
        10 => req(b"READ block="),
        11 => req(b"READ block= 7"),
        12 => req(b"READ block=7 "),
        13 => req(b"read block=7"),
        14 => req(b"READ_RANGE start=0 count=1"),
        15 => req(b"READ_RANGE start=7 count=1"),
        16 => req(b"READ_RANGE start=8 count=1"),
        17 => req(b"READ_RANGE start=3 count=0"),
        18 => req(b"READ_RANGE start=0 count=2"),
        19 => req(b"READ_RANGE start=0 count=18446744073709551616"),
        20 => req(b"READ_RANGE start=18446744073709551616 count=1"),
        21 => req(b"READ_RANGE start=3"),
        22 => req(b"READ_RANGE start=3,count=1"),
        23 => req(b"READ_RANGE start=3 count=1 "),
        24 => req(b""),
        25 => req(b"   "),
        26 => req(b"READ block=\x007"),
        27 => req(&[0xff, 0xfe, 0x00, 0x41]),
        28 => {
            // 64 bytes exactly: "READ block=" + 53 digits (overflow).
            let mut r = b"READ block=".to_vec();
            r.extend(std::iter::repeat_n(b'9', REQ_MAX - r.len()));
            vec![r]
        }
        _ => {
            // 65 bytes: one past the transport bound.
            let mut r = b"READ block=".to_vec();
            r.extend(std::iter::repeat_n(b'1', REQ_MAX + 1 - r.len()));
            vec![r]
        }
    }
}

// ---- Deterministic decoding ---------------------------------------------

const NUMBER_STRINGS: [&[u8]; 12] = [
    b"0",
    b"1",
    b"7",
    b"8",
    b"9",
    b"15",
    b"07",
    b"18446744073709551615",
    b"18446744073709551616",
    b"99999999999999999999999999",
    b"0000003",
    b"",
];

pub fn decode_requests(iteration: u64, input: &[u8]) -> Vec<Vec<u8>> {
    let mut requests = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    for chunk in input.chunks(4) {
        if requests.len() == MAX_STORAGE_OPS_PER_CASE {
            break;
        }
        let family = chunk[0];
        let number = NUMBER_STRINGS[chunk.get(1).copied().unwrap_or(0) as usize % 12];
        let count = NUMBER_STRINGS[chunk.get(2).copied().unwrap_or(0) as usize % 12];
        let mutator = chunk.get(3).copied().unwrap_or(family);

        let mut request: Vec<u8> = match family % 8 {
            0 => P_INFO.to_vec(),
            1 => {
                let mut r = P_READ.to_vec();
                r.extend_from_slice(number);
                r
            }
            2 => {
                let mut r = P_RANGE.to_vec();
                r.extend_from_slice(number);
                r.extend_from_slice(P_COUNT);
                r.extend_from_slice(count);
                r
            }
            3 => {
                let mut r = P_RANGE.to_vec();
                r.extend_from_slice(number);
                r
            }
            4 => vec![mutator; (mutator as usize % (REQ_MAX + 4)).max(1)],
            5 => Vec::new(),
            6 => {
                let mut r = P_READ.to_vec();
                r.extend_from_slice(number);
                r.push(b' ');
                r
            }
            _ => {
                let mut r = P_READ.to_vec();
                r.extend_from_slice(number);
                r
            }
        };
        // One deterministic mutation on top of the template.
        match mutator % 5 {
            0 => {}
            1 => {
                if !request.is_empty() {
                    let position = mutator as usize % request.len();
                    request[position] ^= 0x20;
                }
            }
            2 => {
                let keep = mutator as usize % (request.len() + 1);
                request.truncate(keep);
            }
            3 => {
                let position = mutator as usize % (request.len() + 1);
                request.insert(position, 0);
            }
            _ => request.push(mutator),
        }
        requests.push(request);
    }
    requests
}

// ---- Fuzz target ---------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct StorageTarget;

impl FuzzTarget for StorageTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure(
                "storage target received a case for another target",
            );
        }
        evaluate_requests(&decode_requests(case.iteration, &case.input))
    }
}

pub fn evaluate_requests(requests: &[Vec<u8>]) -> CaseResult {
    if requests.len() > MAX_STORAGE_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "storage request count {} exceeds bound {MAX_STORAGE_OPS_PER_CASE}",
            requests.len()
        ));
    }
    let first = match run_caught(requests) {
        Ok(outcomes) => outcomes,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    let second = match run_caught(requests) {
        Ok(outcomes) => outcomes,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    if first != second {
        return CaseResult::invariant_failure(
            "STOR-INV-005: identical request streams produced different replies",
        );
    }
    CaseResult::safe_reject("storage request sequence completed without an invariant failure")
}

fn run_caught(requests: &[Vec<u8>]) -> Result<Vec<Outcome>, String> {
    match catch_unwind(AssertUnwindSafe(|| run_once(requests))) {
        Ok(result) => result,
        Err(_) => Err(
            "STOR-INV-006: user-controlled storage request triggered a host-reachable panic"
                .to_string(),
        ),
    }
}

fn run_once(requests: &[Vec<u8>]) -> Result<Vec<Outcome>, String> {
    let mut outcomes = Vec::with_capacity(requests.len());
    for request in requests {
        let outcome = storage_reply(request);
        check_invariants(request, &outcome)?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

pub fn check_invariants(request: &[u8], outcome: &Outcome) -> Result<(), String> {
    let expected = expected_outcome(request);
    match outcome {
        Outcome::TransportReject => {
            if expected != Expected::Transport {
                return Err(format!(
                    "STOR-INV-001: in-bound request {:?} was transport-rejected",
                    String::from_utf8_lossy(request)
                ));
            }
            Ok(())
        }
        Outcome::Ignored => {
            if expected != Expected::Ignored {
                return Err(format!(
                    "STOR-INV-003: request {:?} was silently ignored instead of answered",
                    String::from_utf8_lossy(request)
                ));
            }
            Ok(())
        }
        Outcome::Reply(reply) => {
            if reply.len() > REPLY_MAX {
                return Err(format!(
                    "STOR-INV-001: reply exceeds the {REPLY_MAX}-byte bound ({} bytes)",
                    reply.len()
                ));
            }
            let matches = match expected {
                Expected::Transport | Expected::Ignored => false,
                Expected::Info => reply == R_INFO,
                Expected::Data(block) => {
                    let mut wanted = R_DATA.to_vec();
                    wanted.extend_from_slice(BLOCKS[block]);
                    reply == &wanted
                }
                Expected::BadBlock => reply == E_BLOCK,
                Expected::TooMany => reply == E_MANY,
                Expected::Malformed => reply == E_MAL,
            };
            if !matches {
                return Err(format!(
                    "STOR-INV-002/003/007: request {:?} expected {expected:?} but replied {:?}",
                    String::from_utf8_lossy(request),
                    String::from_utf8_lossy(reply)
                ));
            }
            Ok(())
        }
    }
}

// ---- Tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResultClass;

    fn reply(request: &[u8]) -> Vec<u8> {
        match storage_reply(request) {
            Outcome::Reply(reply) => reply,
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn info_and_valid_reads_return_exact_content() {
        assert_eq!(reply(b"INFO"), R_INFO);
        for (block, content) in BLOCKS.iter().enumerate() {
            let request = format!("READ block={block}");
            let mut wanted = R_DATA.to_vec();
            wanted.extend_from_slice(content);
            assert_eq!(reply(request.as_bytes()), wanted, "block {block}");
        }
        assert_eq!(reply(b"READ block=8"), E_BLOCK);
    }

    // The AXIOM-ROBUST-006 regression: overflowing decimals must never
    // alias a valid block (the wrapping runtime returned block 0 for
    // 2^64 before AXIOM-ROBUST-006B).
    #[test]
    fn overflowing_block_numbers_are_malformed_not_aliased() {
        for request in [
            b"READ block=18446744073709551616".as_slice(),
            b"READ block=99999999999999999999999999",
            b"READ_RANGE start=18446744073709551616 count=1",
            b"READ_RANGE start=0 count=18446744073709551616",
            // Wrapping previously made this look like block 0 too.
            b"READ block=18446744073709551624",
        ] {
            let got = reply(request);
            assert_eq!(got, E_MAL, "request {:?}", String::from_utf8_lossy(request));
            assert!(
                !got.starts_with(R_DATA),
                "overflow aliased block data for {:?}",
                String::from_utf8_lossy(request)
            );
        }
        // u64::MAX is 20 digits, so the digit-count rule rejects it too.
        assert_eq!(reply(b"READ block=18446744073709551615"), E_MAL);
    }

    #[test]
    fn malformed_reads_are_rejected_with_exact_errors() {
        for request in [
            b"READ block=".as_slice(),
            b"READ block=-1",
            b"READ block= 7",
            b"READ block=7 ",
            b"READ block=7x",
            b"read block=7",
            b"INFO ",
            b"   ",
            b"READ block=\x007",
        ] {
            assert_eq!(
                reply(request),
                E_MAL,
                "request {:?}",
                String::from_utf8_lossy(request)
            );
        }
    }

    // The live service tests `if r <= 0 { continue }`, so a zero-length
    // message is consumed without any reply — modelling it as an error
    // reply would misrepresent the runtime.
    #[test]
    fn empty_request_produces_no_reply() {
        assert_eq!(storage_reply(b""), Outcome::Ignored);
        assert_eq!(expected_outcome(b""), Expected::Ignored);
    }

    // Leading zeros are accepted inside the digit budget and rejected
    // past it, exactly as the digit-count rule specifies.
    #[test]
    fn digit_count_rule_governs_leading_zeros() {
        let mut wanted = R_DATA.to_vec();
        wanted.extend_from_slice(BLOCKS[7]);
        assert_eq!(reply(b"READ block=0000007"), wanted, "7 digits accepted");
        assert_eq!(
            reply(b"READ block=0000000000000000007"),
            wanted,
            "19 digits accepted"
        );
        assert_eq!(
            reply(b"READ block=00000000000000000007"),
            E_MAL,
            "20 digits rejected even though the value is small"
        );
    }

    #[test]
    fn range_requests_follow_single_block_rules() {
        let mut wanted = R_DATA.to_vec();
        wanted.extend_from_slice(BLOCKS[7]);
        assert_eq!(reply(b"READ_RANGE start=7 count=1"), wanted);
        assert_eq!(reply(b"READ_RANGE start=8 count=1"), E_BLOCK);
        assert_eq!(reply(b"READ_RANGE start=3 count=0"), E_MANY);
        assert_eq!(reply(b"READ_RANGE start=0 count=2"), E_MANY);
        assert_eq!(reply(b"READ_RANGE start=3"), E_MAL);
        assert_eq!(reply(b"READ_RANGE start=3,count=1"), E_MAL);
        assert_eq!(reply(b"READ_RANGE start=3 count=1 "), E_MAL);
        assert_eq!(reply(b"READ_RANGE start= count=1"), E_MAL);
    }

    #[test]
    fn transport_bound_is_enforced() {
        let exact = vec![b'A'; REQ_MAX];
        assert_eq!(storage_reply(&exact), Outcome::Reply(E_MAL.to_vec()));
        let oversize = vec![b'A'; REQ_MAX + 1];
        assert_eq!(storage_reply(&oversize), Outcome::TransportReject);
    }

    #[test]
    fn replies_never_exceed_the_reply_bound() {
        for block in BLOCKS {
            assert!(R_DATA.len() + block.len() <= REPLY_MAX);
            assert!(block.len() <= BLOCK_SIZE);
        }
        assert!(R_INFO.len() <= REPLY_MAX);
    }

    #[test]
    fn checked_parse_boundaries() {
        assert_eq!(parse_dec_checked(b"0", 0), (Some(0), 1));
        // 19 digits is the accepted maximum; 20 is rejected whatever the
        // value, and the scan position is reported either way so the
        // caller's trailing-junk check stays exact.
        assert_eq!(
            parse_dec_checked(b"9999999999999999999", 0),
            (Some(9_999_999_999_999_999_999), 19)
        );
        assert_eq!(parse_dec_checked(b"18446744073709551615", 0), (None, 20));
        assert_eq!(parse_dec_checked(b"18446744073709551616", 0), (None, 20));
        assert_eq!(parse_dec_checked(b"00000000000000000000", 0), (None, 20));
        assert_eq!(parse_dec_checked(b"", 0), (None, 0));
        assert_eq!(parse_dec_checked(b"x1", 0), (None, 0));
        assert_eq!(parse_dec_checked(b"12x", 0), (Some(12), 2));
        assert_eq!(
            parse_dec_checked(b"99999999999999999999x", 0),
            (None, 20),
            "position still lands on the trailing junk"
        );
    }

    #[test]
    fn boundary_bank_is_unique_and_invariant_clean() {
        for (index, name) in FIXED_SCENARIO_NAMES.iter().enumerate() {
            for other in FIXED_SCENARIO_NAMES.iter().skip(index + 1) {
                assert_ne!(name, other);
            }
        }
        for index in 0..MANDATORY_SCENARIOS {
            let requests = mandatory_scenario(index);
            assert!(!requests.is_empty());
            let result = evaluate_requests(&requests);
            assert_ne!(
                result.class,
                ResultClass::KernelInvariantFailure,
                "scenario {} ({}) failed: {}",
                index,
                FIXED_SCENARIO_NAMES[index as usize],
                result.reason
            );
        }
    }

    #[test]
    fn every_decoded_request_stream_is_invariant_clean() {
        for seed_byte in 0..=u8::MAX {
            let input = [seed_byte, seed_byte.wrapping_mul(3), 251, seed_byte ^ 0x5a];
            let requests = decode_requests(u64::from(seed_byte), &input);
            assert!(requests.len() <= MAX_STORAGE_OPS_PER_CASE);
            let result = evaluate_requests(&requests);
            assert_ne!(
                result.class,
                ResultClass::KernelInvariantFailure,
                "byte {seed_byte}: {}",
                result.reason
            );
        }
    }

    #[test]
    fn decoding_and_replies_are_deterministic() {
        let requests = decode_requests(9, &[1, 8, 3, 0, 2, 9, 1, 4]);
        assert_eq!(requests, decode_requests(9, &[1, 8, 3, 0, 2, 9, 1, 4]));
        assert_eq!(evaluate_requests(&requests), evaluate_requests(&requests));
    }
}
