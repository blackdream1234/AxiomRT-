//! Deterministic adversarial target for the fs_service protocol.
//!
//! Models the docs/28 §3/§4 read-only filesystem protocol exactly as
//! the U-mode fs_service implements it (os_boot.rs fs_body), including
//! the docs/33 storage-backed /bin record bridge: `CAT /bin/<app>.app`
//! performs a nested storage `READ block=<4|5|6>` through the storage
//! model of this crate and strips the `OK data=` frame; any storage
//! failure answers `ERR not_found` — fs never invents content.
//!
//! An independent std-based path oracle re-derives the expected reply
//! for every request; divergence, unknown-path acceptance, content
//! aliasing, out-of-bound replies, or panics are
//! KERNEL_INVARIANT_FAILUREs. Host-model evidence only — the live
//! U-mode services and their IPC transport remain covered by the
//! filesystem/storage/restricted-loader QEMU tests.

use crate::targets::storage;
use crate::{CaseResult, FuzzCase, FuzzTarget};
use kernel::ipc::MSG_MAX_BYTES;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NAME: &str = "fs";
pub const MAX_FS_OPS_PER_CASE: usize = 16;

/// Request bound: fs_service receives into a 64-byte buffer (docs/28
/// §3). A larger message is refused by the bounded IPC path itself —
/// the sender is answered ERR_MSG_TOO_LARGE and the service never sees
/// the bytes.
pub const REQ_MAX: usize = 64;
/// Reply bound: the global IPC message maximum, imported from the
/// kernel model. Directory listings legitimately exceed 64 bytes since
/// the /bin records landed (docs/33 §3: the listing is 103 bytes),
/// which is why the shell receives fs replies with a 128-byte window.
pub const REPLY_MAX: usize = MSG_MAX_BYTES;

pub const E_NOTFOUND: &[u8] = b"ERR not_found";
pub const E_BADPATH: &[u8] = b"ERR bad_path";

/// LS directory table (docs/28 §5, docs/33 §3; verbatim mirrors).
pub const LS_TABLE: [(&[u8], &[u8]); 5] = [
    (b"/", b"OK etc apps docs bin"),
    (b"/etc", b"OK version limitations"),
    (
        b"/apps",
        b"OK hello.manifest counter.manifest fault_demo.manifest",
    ),
    (b"/docs", b"OK about"),
    (
        b"/bin",
        b"OK hello.app counter.app fault_demo.app invalid_bad_magic.app invalid_bad_cap.app invalid_bad_checksum.app",
    ),
];

/// Static CAT files (docs/28 §5).
pub const CAT_STATIC: [(&[u8], &[u8]); 6] = [
    (
        b"/etc/version",
        b"OK AxiomRT v1.6-storage-backed-loader RISC-V 64 eval stage",
    ),
    (
        b"/etc/limitations",
        b"OK emulator-only read-only evaluation build no cert claim",
    ),
    (
        b"/apps/hello.manifest",
        b"OK hello: prio=2 caps=console restart=rerun",
    ),
    (
        b"/apps/counter.manifest",
        b"OK counter: prio=2 caps=console restart=rerun",
    ),
    (
        b"/apps/fault_demo.manifest",
        b"OK fault_demo: prio=2 caps=none restart=rerun",
    ),
    (
        b"/docs/about",
        b"OK AxiomRT v1.6-storage-backed-loader see docs/INDEX.md",
    ),
];

/// Deliberately corrupt fs-static /bin fixtures (docs/33 §3).
pub const CAT_FIXTURES: [(&[u8], &[u8]); 3] = [
    (
        b"/bin/invalid_bad_magic.app",
        b"BXAPP1 invalid_bad_magic 0 4096 4096 1 none 8192 0000",
    ),
    (
        b"/bin/invalid_bad_cap.app",
        b"AXAPP1 invalid_bad_cap 0 4096 4096 1 mmio 8192 0d18",
    ),
    (
        b"/bin/invalid_bad_checksum.app",
        b"AXAPP1 invalid_bad_checksum 0 4096 4096 1 none 8192 0000",
    ),
];

/// Storage-backed /bin records (docs/33 §6): path → storage block.
pub const CAT_STORAGE_RECORDS: [(&[u8], u64); 3] = [
    (b"/bin/hello.app", 4),
    (b"/bin/counter.app", 5),
    (b"/bin/fault_demo.app", 6),
];

/// The storage-forwarded path (docs/29 §7): reply passed on verbatim.
pub const PATH_STORAGE_VERSION: &[u8] = b"/storage/version";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    TransportReject,
    /// Zero-length message: `fs_body` tests `if r <= 0 { continue }` and
    /// loops without answering, so the service produces **no reply at
    /// all** — not an `ERR bad_path`. No in-tree client sends one (every
    /// sender writes at least the `LS `/`CAT ` opcode).
    Ignored,
    Reply(Vec<u8>),
}

/// The fs_service protocol model (docs/28 §3; prefix-order mirror of
/// fs_body). `storage_up` injects the nested-IPC failure path: when
/// false, every storage send fails and record fetches answer
/// `ERR not_found`.
pub fn fs_reply(request: &[u8], storage_up: bool) -> Outcome {
    if request.len() > REQ_MAX {
        return Outcome::TransportReject;
    }
    if request.is_empty() {
        return Outcome::Ignored;
    }
    if let Some(path) = strip_prefix(request, b"LS ") {
        for (known, listing) in LS_TABLE {
            if path == known {
                return Outcome::Reply(listing.to_vec());
            }
        }
        return Outcome::Reply(E_NOTFOUND.to_vec());
    }
    if let Some(path) = strip_prefix(request, b"CAT ") {
        for (known, content) in CAT_STATIC {
            if path == known {
                return Outcome::Reply(content.to_vec());
            }
        }
        for (known, block) in CAT_STORAGE_RECORDS {
            if path == known {
                return Outcome::Reply(fetch_record(block, storage_up));
            }
        }
        for (known, fixture) in CAT_FIXTURES {
            if path == known {
                return Outcome::Reply(fixture.to_vec());
            }
        }
        if path == PATH_STORAGE_VERSION {
            // Nested storage read forwarded verbatim (docs/29 §7).
            if !storage_up {
                return Outcome::Reply(E_NOTFOUND.to_vec());
            }
            return match storage::storage_reply(b"READ block=1") {
                storage::Outcome::Reply(reply) if !reply.is_empty() => Outcome::Reply(reply),
                _ => Outcome::Reply(E_NOTFOUND.to_vec()),
            };
        }
        return Outcome::Reply(E_NOTFOUND.to_vec());
    }
    Outcome::Reply(E_BADPATH.to_vec())
}

/// Fetch one storage-backed record and strip the `OK data=` frame
/// (docs/33 §6). Any failure — storage down, error reply, or a reply
/// no longer than the frame — answers `ERR not_found`.
fn fetch_record(block: u64, storage_up: bool) -> Vec<u8> {
    if !storage_up {
        return E_NOTFOUND.to_vec();
    }
    let request = format!("READ block={block}");
    match storage::storage_reply(request.as_bytes()) {
        storage::Outcome::Reply(reply)
            if reply.len() > storage::R_DATA.len() && reply.starts_with(storage::R_DATA) =>
        {
            reply[storage::R_DATA.len()..].to_vec()
        }
        _ => E_NOTFOUND.to_vec(),
    }
}

fn strip_prefix<'a>(request: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    if request.len() >= prefix.len() && &request[..prefix.len()] == prefix {
        Some(&request[prefix.len()..])
    } else {
        None
    }
}

// ---- Independent path oracle --------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expected {
    Transport,
    /// Zero-length message: consumed and ignored, no reply.
    Ignored,
    Listing(usize),
    StaticFile(usize),
    Fixture(usize),
    /// Storage-backed record: exact block content when storage is up,
    /// `ERR not_found` when it is down.
    Record(usize),
    StorageVersion,
    NotFound,
    BadPath,
}

pub fn expected_outcome(request: &[u8]) -> Expected {
    if request.len() > REQ_MAX {
        return Expected::Transport;
    }
    if request.is_empty() {
        return Expected::Ignored;
    }
    let text: Vec<u8> = request.to_vec();
    if let Some(path) = text.strip_prefix(b"LS ".as_slice()) {
        if let Some(index) = LS_TABLE.iter().position(|(known, _)| *known == path) {
            return Expected::Listing(index);
        }
        return Expected::NotFound;
    }
    if let Some(path) = text.strip_prefix(b"CAT ".as_slice()) {
        if let Some(index) = CAT_STATIC.iter().position(|(known, _)| *known == path) {
            return Expected::StaticFile(index);
        }
        if let Some(index) = CAT_STORAGE_RECORDS
            .iter()
            .position(|(known, _)| *known == path)
        {
            return Expected::Record(index);
        }
        if let Some(index) = CAT_FIXTURES.iter().position(|(known, _)| *known == path) {
            return Expected::Fixture(index);
        }
        if path == PATH_STORAGE_VERSION {
            return Expected::StorageVersion;
        }
        return Expected::NotFound;
    }
    Expected::BadPath
}

// ---- Fixed boundary bank -------------------------------------------------

/// One decoded fuzz operation: a request, or toggling the modeled
/// storage dependency (the nested-IPC failure path).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FsOp {
    Request(Vec<u8>),
    SetStorage(bool),
}

pub const FIXED_SCENARIO_NAMES: [&str; 32] = [
    "ls_root",
    "ls_etc",
    "ls_apps",
    "ls_docs",
    "ls_bin",
    "ls_unknown",
    "ls_relative",
    "ls_trailing_slash",
    "ls_no_space",
    "cat_version",
    "cat_limitations",
    "cat_manifest_hello",
    "cat_about",
    "cat_bin_hello_record",
    "cat_bin_counter_record",
    "cat_bin_fault_record",
    "cat_fixture_bad_magic",
    "cat_fixture_bad_cap",
    "cat_fixture_bad_checksum",
    "cat_storage_version",
    "cat_unknown",
    "cat_directory",
    "cat_dotdot",
    "cat_case_mutated",
    "cat_prefix_extended",
    "path_len_59",
    "path_len_60_max_fit",
    "transport_oversize",
    "empty_request_ignored",
    "whitespace_request",
    "nul_in_path",
    "storage_down_record_fetch",
];

const MANDATORY_SCENARIOS: u64 = FIXED_SCENARIO_NAMES.len() as u64;

fn mandatory_scenario(index: u64) -> Vec<FsOp> {
    let req = |bytes: &[u8]| vec![FsOp::Request(bytes.to_vec())];
    match index {
        0 => req(b"LS /"),
        1 => req(b"LS /etc"),
        2 => req(b"LS /apps"),
        3 => req(b"LS /docs"),
        4 => req(b"LS /bin"),
        5 => req(b"LS /nope"),
        6 => req(b"LS etc"),
        7 => req(b"LS /etc/"),
        8 => req(b"LS"),
        9 => req(b"CAT /etc/version"),
        10 => req(b"CAT /etc/limitations"),
        11 => req(b"CAT /apps/hello.manifest"),
        12 => req(b"CAT /docs/about"),
        13 => req(b"CAT /bin/hello.app"),
        14 => req(b"CAT /bin/counter.app"),
        15 => req(b"CAT /bin/fault_demo.app"),
        16 => req(b"CAT /bin/invalid_bad_magic.app"),
        17 => req(b"CAT /bin/invalid_bad_cap.app"),
        18 => req(b"CAT /bin/invalid_bad_checksum.app"),
        19 => req(b"CAT /storage/version"),
        20 => req(b"CAT /etc/nope"),
        21 => req(b"CAT /etc"),
        22 => req(b"CAT /../etc/version"),
        23 => req(b"cat /etc/version"),
        24 => req(b"CAT /etc/versionX"),
        25 => {
            let mut r = b"CAT /".to_vec();
            r.extend(std::iter::repeat_n(b'a', 58));
            vec![FsOp::Request(r)] // 59-byte path (docs/28 §4 maximum)
        }
        26 => {
            let mut r = b"CAT /".to_vec();
            r.extend(std::iter::repeat_n(b'a', 59));
            vec![FsOp::Request(r)] // 60-byte path: last fitting request
        }
        27 => {
            let mut r = b"CAT /".to_vec();
            r.extend(std::iter::repeat_n(b'a', 60));
            vec![FsOp::Request(r)] // 65-byte request: past the transport
        }
        28 => req(b""),
        29 => req(b"   "),
        30 => req(b"CAT /etc/ver\x00sion"),
        _ => vec![
            FsOp::SetStorage(false),
            FsOp::Request(b"CAT /bin/hello.app".to_vec()),
            FsOp::Request(b"CAT /storage/version".to_vec()),
            FsOp::SetStorage(true),
            FsOp::Request(b"CAT /bin/hello.app".to_vec()),
        ],
    }
}

// ---- Deterministic decoding ---------------------------------------------

const PATHS: [&[u8]; 14] = [
    b"/",
    b"/etc",
    b"/apps",
    b"/docs",
    b"/bin",
    b"/etc/version",
    b"/etc/limitations",
    b"/apps/hello.manifest",
    b"/docs/about",
    b"/bin/hello.app",
    b"/bin/invalid_bad_magic.app",
    b"/storage/version",
    b"/unknown",
    b"",
];

pub fn decode_operations(iteration: u64, input: &[u8]) -> Vec<FsOp> {
    let mut operations = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    for chunk in input.chunks(4) {
        if operations.len() == MAX_FS_OPS_PER_CASE {
            break;
        }
        let family = chunk[0];
        let path = PATHS[chunk.get(1).copied().unwrap_or(0) as usize % PATHS.len()];
        let mutator = chunk.get(2).copied().unwrap_or(family);
        let extra = chunk.get(3).copied().unwrap_or(mutator);

        let operation = match family % 8 {
            0 => FsOp::SetStorage(extra & 1 == 0),
            1 => FsOp::Request(build(b"LS ", path)),
            2 | 3 => FsOp::Request(build(b"CAT ", path)),
            4 => FsOp::Request(path.to_vec()),
            5 => FsOp::Request(vec![extra; (extra as usize % (REQ_MAX + 4)).max(1)]),
            _ => {
                let mut request = build(if family & 1 == 0 { b"CAT " } else { b"LS " }, path);
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
                FsOp::Request(request)
            }
        };
        operations.push(operation);
    }
    operations
}

fn build(prefix: &[u8], path: &[u8]) -> Vec<u8> {
    let mut request = prefix.to_vec();
    request.extend_from_slice(path);
    request
}

// ---- Fuzz target ---------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct FsTarget;

impl FuzzTarget for FsTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure("fs target received a case for another target");
        }
        evaluate_operations(&decode_operations(case.iteration, &case.input))
    }
}

pub fn evaluate_operations(operations: &[FsOp]) -> CaseResult {
    if operations.len() > MAX_FS_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "fs operation count {} exceeds bound {MAX_FS_OPS_PER_CASE}",
            operations.len()
        ));
    }
    let first = match run_caught(operations) {
        Ok(outcomes) => outcomes,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    let second = match run_caught(operations) {
        Ok(outcomes) => outcomes,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    if first != second {
        return CaseResult::invariant_failure(
            "FS-INV-005: identical request streams produced different replies",
        );
    }
    CaseResult::safe_reject("fs request sequence completed without an invariant failure")
}

fn run_caught(operations: &[FsOp]) -> Result<Vec<Outcome>, String> {
    match catch_unwind(AssertUnwindSafe(|| run_once(operations))) {
        Ok(result) => result,
        Err(_) => Err(
            "FS-INV-006: user-controlled fs request triggered a host-reachable panic".to_string(),
        ),
    }
}

fn run_once(operations: &[FsOp]) -> Result<Vec<Outcome>, String> {
    let mut storage_up = true;
    let mut outcomes = Vec::with_capacity(operations.len());
    for operation in operations {
        match operation {
            FsOp::SetStorage(up) => storage_up = *up,
            FsOp::Request(request) => {
                let outcome = fs_reply(request, storage_up);
                check_invariants(request, storage_up, &outcome)?;
                outcomes.push(outcome);
            }
        }
    }
    Ok(outcomes)
}

pub fn check_invariants(request: &[u8], storage_up: bool, outcome: &Outcome) -> Result<(), String> {
    let expected = expected_outcome(request);
    match outcome {
        Outcome::TransportReject => {
            if expected != Expected::Transport {
                return Err(format!(
                    "FS-INV-001: in-bound request {:?} was transport-rejected",
                    String::from_utf8_lossy(request)
                ));
            }
            Ok(())
        }
        Outcome::Ignored => {
            if expected != Expected::Ignored {
                return Err(format!(
                    "FS-INV-002: request {:?} was silently ignored instead of answered",
                    String::from_utf8_lossy(request)
                ));
            }
            Ok(())
        }
        Outcome::Reply(reply) => {
            if reply.len() > REPLY_MAX {
                return Err(format!(
                    "FS-INV-001: reply exceeds the {REPLY_MAX}-byte bound ({} bytes)",
                    reply.len()
                ));
            }
            let wanted: Vec<u8> = match expected {
                Expected::Transport | Expected::Ignored => {
                    return Err(format!(
                        "FS-INV-001/002: request {:?} should have produced no reply",
                        String::from_utf8_lossy(request)
                    ))
                }
                Expected::Listing(index) => LS_TABLE[index].1.to_vec(),
                Expected::StaticFile(index) => CAT_STATIC[index].1.to_vec(),
                Expected::Fixture(index) => CAT_FIXTURES[index].1.to_vec(),
                Expected::Record(index) => {
                    if storage_up {
                        storage::BLOCKS[CAT_STORAGE_RECORDS[index].1 as usize].to_vec()
                    } else {
                        E_NOTFOUND.to_vec()
                    }
                }
                Expected::StorageVersion => {
                    if storage_up {
                        let mut wanted = storage::R_DATA.to_vec();
                        wanted.extend_from_slice(storage::BLOCKS[1]);
                        wanted
                    } else {
                        E_NOTFOUND.to_vec()
                    }
                }
                Expected::NotFound => E_NOTFOUND.to_vec(),
                Expected::BadPath => E_BADPATH.to_vec(),
            };
            if reply != &wanted {
                return Err(format!(
                    "FS-INV-002/003/004/007: request {:?} (storage_up={storage_up}) expected \
                     {:?} but replied {:?}",
                    String::from_utf8_lossy(request),
                    String::from_utf8_lossy(&wanted),
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
        match fs_reply(request, true) {
            Outcome::Reply(reply) => reply,
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn every_known_path_answers_its_exact_content() {
        for (path, listing) in LS_TABLE {
            assert_eq!(reply(&build(b"LS ", path)), listing);
        }
        for (path, content) in CAT_STATIC {
            assert_eq!(reply(&build(b"CAT ", path)), content);
        }
        for (path, fixture) in CAT_FIXTURES {
            assert_eq!(reply(&build(b"CAT ", path)), fixture);
        }
        for (path, block) in CAT_STORAGE_RECORDS {
            assert_eq!(
                reply(&build(b"CAT ", path)),
                storage::BLOCKS[block as usize],
                "record for {:?}",
                String::from_utf8_lossy(path)
            );
        }
        // /storage/version forwards the raw storage reply (docs/29 §7).
        let mut wanted = storage::R_DATA.to_vec();
        wanted.extend_from_slice(storage::BLOCKS[1]);
        assert_eq!(reply(b"CAT /storage/version"), wanted);
    }

    #[test]
    fn unknown_relative_and_mutated_paths_are_rejected() {
        for request in [
            b"LS /nope".as_slice(),
            b"LS etc",
            b"LS /etc/",
            b"LS //",
            b"CAT /etc/nope",
            b"CAT /etc",
            b"CAT /../etc/version",
            b"CAT /etc/versionX",
            b"CAT /etc/versio",
            b"CAT /ETC/version",
            b"CAT /etc/ver\x00sion",
            b"CAT /bin/hello.ap",
        ] {
            assert_eq!(
                reply(request),
                E_NOTFOUND,
                "request {:?}",
                String::from_utf8_lossy(request)
            );
        }
        for request in [
            b"LS".as_slice(),
            b"CAT",
            b"ls /",
            b"cat /etc/version",
            b"   ",
            b"DELETE /etc/version",
            b"LS/",
            b"CAT/etc/version",
        ] {
            assert_eq!(
                reply(request),
                E_BADPATH,
                "request {:?}",
                String::from_utf8_lossy(request)
            );
        }
    }

    // FS-INV-004: storage failure maps to ERR not_found, never partial
    // or invented content, and recovery restores exact records.
    #[test]
    fn storage_failure_never_invents_record_content() {
        for path in [b"CAT /bin/hello.app".as_slice(), b"CAT /storage/version"] {
            let down = fs_reply(path, false);
            assert_eq!(down, Outcome::Reply(E_NOTFOUND.to_vec()));
        }
        let up = fs_reply(b"CAT /bin/hello.app", true);
        assert_eq!(up, Outcome::Reply(storage::BLOCKS[4].to_vec()));
    }

    // The live service tests `if r <= 0 { continue }`, so a zero-length
    // message is consumed without any reply.
    #[test]
    fn empty_request_produces_no_reply() {
        assert_eq!(fs_reply(b"", true), Outcome::Ignored);
        assert_eq!(expected_outcome(b""), Expected::Ignored);
    }

    #[test]
    fn path_length_boundaries() {
        // 59- and 60-byte unknown paths fit the transport: not_found.
        for extra in [58usize, 59] {
            let mut request = b"CAT /".to_vec();
            request.extend(std::iter::repeat_n(b'a', extra));
            assert!(request.len() <= REQ_MAX);
            assert_eq!(reply(&request), E_NOTFOUND, "path len {}", extra + 1);
        }
        // 65-byte request exceeds the transport bound.
        let mut oversize = b"CAT /".to_vec();
        oversize.extend(std::iter::repeat_n(b'a', 60));
        assert_eq!(fs_reply(&oversize, true), Outcome::TransportReject);
    }

    #[test]
    fn boundary_bank_is_unique_and_invariant_clean() {
        for (index, name) in FIXED_SCENARIO_NAMES.iter().enumerate() {
            for other in FIXED_SCENARIO_NAMES.iter().skip(index + 1) {
                assert_ne!(name, other);
            }
        }
        for index in 0..MANDATORY_SCENARIOS {
            let operations = mandatory_scenario(index);
            assert!(!operations.is_empty());
            assert!(operations.len() <= MAX_FS_OPS_PER_CASE);
            let result = evaluate_operations(&operations);
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
    fn every_decoded_operation_stream_is_invariant_clean() {
        for seed_byte in 0..=u8::MAX {
            let input = [seed_byte, seed_byte.wrapping_mul(7), 13, seed_byte ^ 0xa5];
            let operations = decode_operations(u64::from(seed_byte), &input);
            assert!(operations.len() <= MAX_FS_OPS_PER_CASE);
            let result = evaluate_operations(&operations);
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
        let operations = decode_operations(21, &[6, 9, 3, 1, 0, 2, 5, 5]);
        assert_eq!(operations, decode_operations(21, &[6, 9, 3, 1, 0, 2, 5, 5]));
        assert_eq!(
            evaluate_operations(&operations),
            evaluate_operations(&operations)
        );
    }

    #[test]
    fn replies_never_exceed_the_reply_bound() {
        for (_, listing) in LS_TABLE {
            assert!(listing.len() <= REPLY_MAX);
        }
        for (_, content) in CAT_STATIC {
            assert!(content.len() <= REPLY_MAX);
        }
        for (_, fixture) in CAT_FIXTURES {
            assert!(fixture.len() <= REPLY_MAX);
        }
    }
}
