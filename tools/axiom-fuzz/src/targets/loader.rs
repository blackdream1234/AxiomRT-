//! Deterministic adversarial target for the restricted app loader.
//!
//! Models the docs/32 §6 AXAPP1 validation contract exactly as the
//! U-mode `app_loader_service` implements it (os_boot.rs `ld_validate`,
//! `ld_load`, `ld_run`, `ld_unload`, `ld_state`), plus the docs/33 §7
//! fetch path, which is routed through this crate's fs model so the
//! `/bin` record vocabulary comes from one source of truth.
//!
//! Two things make this more than a re-implementation:
//!
//! * mapping admission is decided by the **real** kernel host API
//!   `kernel::loader::admit_image_mapping`, so LD-INV-005 checks that
//!   every record the loader accepts describes a layout the kernel
//!   mapping mechanism would actually admit (W^X by separation, entry
//!   inside text, bounded image, one stack page, user-space span);
//! * an independent field-splitting oracle re-derives the verdict for
//!   every record, so a *wrong verdict* — not just a crash — fails.
//!
//! Resolved contract note (AXIOM-ROBUST-007): docs/32 §6 numbers the
//! checks with fields (3) before checksum (4), but the runtime runs the
//! checksum first and says so in its comment ("checked before field
//! parsing so a corrupt record never drives the parser"). The runtime
//! order is intentional and strictly safer; only the error word for a
//! record that is *both* malformed and mis-checksummed differs. This
//! target encodes the runtime order and docs/32 §6 was corrected to
//! record it.

use crate::targets::fs;
use crate::{CaseResult, FuzzCase, FuzzTarget};
use kernel::loader::{admit_image_mapping, ImageLayout};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NAME: &str = "loader";
pub const MAX_LOADER_OPS_PER_CASE: usize = 16;

// ---- Bounds mirrored from the runtime -----------------------------------

/// Record transport bound: `ld_fetch` receives into 64 bytes.
pub const REC_MAX: usize = 64;
/// `ld_validate` rule 1: `rl < LD_MAGIC_LEN + 6` is malformed.
pub const REC_MIN: usize = MAGIC.len() + 6;
/// `ld_fetch` rejects an empty name or one longer than this.
pub const NAME_MAX: usize = 27;
/// Magic + version + the separating space (`LD_MAGIC`).
pub const MAGIC: &[u8] = b"AXAPP1 ";
/// docs/32 §6 rule 5 layout bounds.
pub const TEXT_MAX: u64 = 65536;
pub const RODATA_MAX: u64 = 65536;
pub const IMAGE_MAX: u64 = 131_072;
/// v1.6 stack policy: exactly one page.
pub const STACK_PAGES: u64 = 1;
/// Base VA the restricted images are mapped at (docs/25 §2 user region).
pub const USER_BASE_VA: u64 = 0x1_0000;
/// docs/32 §5 capability vocabulary.
pub const CAP_CONSOLE: &[u8] = b"console";
pub const CAP_NONE: &[u8] = b"none";

// ---- Verdicts (ld_validate result codes) --------------------------------

/// `ld_validate` return codes, named. The numeric values are the
/// runtime's own (0 = valid .. 5 = unknown app).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Valid,
    BadImage,
    BadChecksum,
    DeniedCapability,
    Malformed,
    NotFound,
}

impl Verdict {
    /// The bounded reply the loader sends for this verdict.
    pub fn reply(self) -> &'static [u8] {
        match self {
            Verdict::Valid => b"OK",
            Verdict::BadImage => b"ERR bad_image",
            Verdict::BadChecksum => b"ERR bad_checksum",
            Verdict::DeniedCapability => b"ERR denied_capability",
            Verdict::Malformed => b"ERR malformed",
            Verdict::NotFound => b"ERR not_found",
        }
    }
}

// ---- Known apps and their capability policy (docs/32 §5) ----------------

/// (name, console allowed). `fault_demo` may request only `none`.
pub const APPS: [(&[u8], bool); 3] = [(b"hello", true), (b"counter", true), (b"fault_demo", false)];

fn app_id(name: &[u8]) -> Option<usize> {
    APPS.iter().position(|(known, _)| *known == name)
}

// ---- Record construction ------------------------------------------------

/// Build a well-formed AXAPP1 record with a correct checksum. Used to
/// generate valid records and, by mutating one field at a time, the
/// near-valid ones. The checksum covers every byte before the trailing
/// ` xxxx` (docs/32 §6 rule 4).
pub fn build_record(
    name: &[u8],
    entry: u64,
    text: u64,
    rodata: u64,
    stack: u64,
    caps: &[u8],
    image: u64,
) -> Vec<u8> {
    let mut body = MAGIC.to_vec();
    body.extend_from_slice(name);
    for value in [entry, text, rodata, stack] {
        body.push(b' ');
        body.extend_from_slice(value.to_string().as_bytes());
    }
    body.push(b' ');
    body.extend_from_slice(caps);
    body.push(b' ');
    body.extend_from_slice(image.to_string().as_bytes());
    let sum = checksum(&body);
    body.push(b' ');
    body.extend_from_slice(format!("{sum:04x}").as_bytes());
    body
}

/// 16-bit additive checksum over the given bytes (docs/32 §6 rule 4).
pub fn checksum(bytes: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for byte in bytes {
        sum = (sum + u32::from(*byte)) & 0xffff;
    }
    sum
}

/// A canonical valid record for a known app (matches the runtime's
/// storage blocks 4-6 byte for byte; asserted in the tests).
pub fn canonical_record(app: usize) -> Vec<u8> {
    let (name, console) = APPS[app];
    let caps = if console { CAP_CONSOLE } else { CAP_NONE };
    build_record(name, 0, 4096, 4096, 1, caps, 8192)
}

// ---- The validator model (mirror of ld_validate) ------------------------

/// Mirror of `ld_validate`, in its real order: length, magic, checksum
/// tail, checksum, name match, positional fields, layout, capability,
/// known app.
pub fn validate(record: &[u8], requested: &[u8]) -> Verdict {
    let rl = record.len();
    // (1) length: room for the header and the ` xxxx` tail.
    if rl < REC_MIN {
        return Verdict::Malformed;
    }
    // (2) magic + version.
    if !record.starts_with(MAGIC) {
        return Verdict::BadImage;
    }
    // (4) checksum, before any field parsing.
    if record[rl - 5] != b' ' {
        return Verdict::Malformed;
    }
    let Some(want) = parse_hex4(&record[rl - 4..]) else {
        return Verdict::Malformed;
    };
    if checksum(&record[..rl - 5]) != want {
        return Verdict::BadChecksum;
    }
    // (3) positional fields: name, then the numbers and the caps token.
    let i0 = MAGIC.len();
    let i1 = tok_end(record, i0);
    if i1 <= i0 {
        return Verdict::Malformed;
    }
    // A record naming a different app than the request is a wrong
    // image, not a transport error.
    if &record[i0..i1] != requested {
        return Verdict::BadImage;
    }
    let (entry, j1) = num_after(record, i1);
    let (text, j2) = num_after(record, j1);
    let (rodata, j3) = num_after(record, j2);
    let (stack, j4) = num_after(record, j3);
    let (Some(entry), Some(text), Some(rodata), Some(stack)) = (entry, text, rodata, stack) else {
        return Verdict::Malformed;
    };
    if j4 >= rl || record[j4] != b' ' {
        return Verdict::Malformed;
    }
    let c0 = j4 + 1;
    let c1 = tok_end(record, c0);
    if c1 <= c0 {
        return Verdict::Malformed;
    }
    let (image, j5) = num_after(record, c1);
    let Some(image) = image else {
        return Verdict::Malformed;
    };
    // The image-size field must end exactly where the checksum tail
    // begins: no extra fields, no trailing bytes.
    if j5 != rl - 5 {
        return Verdict::Malformed;
    }
    // (5) layout.
    if text == 0
        || text > TEXT_MAX
        || rodata > RODATA_MAX
        || image != text.wrapping_add(rodata)
        || image > IMAGE_MAX
        || entry >= text
        || stack != STACK_PAGES
    {
        return Verdict::BadImage;
    }
    // (6) capability request: known word, within the app's policy.
    let caps = &record[c0..c1];
    let want_console = caps == CAP_CONSOLE;
    if !want_console && caps != CAP_NONE {
        return Verdict::DeniedCapability;
    }
    // (7) known runnable app and its policy.
    match app_id(requested) {
        Some(id) => {
            if want_console && !APPS[id].1 {
                Verdict::DeniedCapability
            } else {
                Verdict::Valid
            }
        }
        None => Verdict::NotFound,
    }
}

/// End of the token starting at `i`: first space, or the record end.
fn tok_end(record: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < record.len() && record[j] != b' ' {
        j += 1;
    }
    j
}

/// Expect a space at `j`, then a decimal number. Mirrors
/// `ld_num_after` + the AXIOM-ROBUST-006B digit-count rule: at most 19
/// digits, otherwise "not a number".
fn num_after(record: &[u8], j: usize) -> (Option<u64>, usize) {
    if j >= record.len() || record[j] != b' ' {
        return (None, j);
    }
    let mut value: u64 = 0;
    let mut index = j + 1;
    let mut digits = 0usize;
    while index < record.len() && record[index].is_ascii_digit() {
        if digits < 19 {
            value = value * 10 + u64::from(record[index] - b'0');
        }
        digits += 1;
        index += 1;
    }
    if digits > 0 && digits <= 19 {
        (Some(value), index)
    } else {
        (None, index)
    }
}

/// Exactly four lowercase hex digits (mirror of `parse_hex4`).
fn parse_hex4(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 4 {
        return None;
    }
    let mut value: u32 = 0;
    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a') + 10,
            _ => return None,
        };
        value = (value << 4) | digit;
    }
    Some(value)
}

// ---- Independent oracle -------------------------------------------------

/// Re-derive the verdict with a different implementation strategy:
/// split the record into fields and check each with std parsing. The
/// contract order is the runtime's (checksum before fields).
pub fn expected_verdict(record: &[u8], requested: &[u8]) -> Verdict {
    let rl = record.len();
    if rl < REC_MIN {
        return Verdict::Malformed;
    }
    if !record.starts_with(MAGIC) {
        return Verdict::BadImage;
    }
    if record[rl - 5] != b' ' {
        return Verdict::Malformed;
    }
    let tail = &record[rl - 4..];
    let Ok(tail_text) = std::str::from_utf8(tail) else {
        return Verdict::Malformed;
    };
    if tail_text.len() != 4
        || !tail_text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Verdict::Malformed;
    }
    let Ok(want) = u32::from_str_radix(tail_text, 16) else {
        return Verdict::Malformed;
    };
    if checksum(&record[..rl - 5]) != want {
        return Verdict::BadChecksum;
    }

    // Body = everything between the magic and the checksum tail. It must
    // split into exactly six single-space-separated fields:
    // name entry text rodata stack caps image.
    let body = &record[MAGIC.len()..rl - 5];
    let fields: Vec<&[u8]> = body.split(|byte| *byte == b' ').collect();
    if fields.len() != 7 || fields.iter().any(|field| field.is_empty()) {
        return Verdict::Malformed;
    }
    if fields[0] != requested {
        return Verdict::BadImage;
    }
    let mut numbers = [0u64; 5];
    for (slot, field) in numbers.iter_mut().zip([1usize, 2, 3, 4, 6]) {
        let Some(value) = decimal(fields[field]) else {
            return Verdict::Malformed;
        };
        *slot = value;
    }
    let [entry, text, rodata, stack, image] = numbers;
    if text == 0
        || text > TEXT_MAX
        || rodata > RODATA_MAX
        || image != text.wrapping_add(rodata)
        || image > IMAGE_MAX
        || entry >= text
        || stack != STACK_PAGES
    {
        return Verdict::BadImage;
    }
    let caps = fields[5];
    let want_console = caps == CAP_CONSOLE;
    if !want_console && caps != CAP_NONE {
        return Verdict::DeniedCapability;
    }
    match app_id(requested) {
        Some(id) if want_console && !APPS[id].1 => Verdict::DeniedCapability,
        Some(_) => Verdict::Valid,
        None => Verdict::NotFound,
    }
}

/// All-digits, at most 19 of them (the AXIOM-ROBUST-006B rule).
fn decimal(field: &[u8]) -> Option<u64> {
    if field.is_empty() || field.len() > 19 || !field.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(field).ok()?.parse().ok()
}

// ---- Loader lifecycle model --------------------------------------------

/// Loader-visible state of one app (`st` byte in the runtime).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppState {
    Available,
    Loaded,
    Running,
}

/// Kernel task state observed through sys_info kind 7 once an app ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    NeverRun,
    Running,
    Exited,
    Faulted,
}

/// Everything a rejected operation must leave untouched.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Protected {
    states: [AppState; 3],
    tasks: [TaskState; 3],
    /// Capability word granted at load time, per app. `None` = no grant.
    granted: [Option<&'static [u8]>; 3],
    starts: [u64; 3],
}

struct LoaderModel {
    protected: Protected,
    replies: Vec<Vec<u8>>,
}

// ---- Operations ---------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoaderOp {
    /// `APP_LOAD <name>` fetching the record through the fs `/bin` path.
    Load {
        name: Vec<u8>,
    },
    /// `APP_LOAD <name>` where the fetched record is a synthesized one.
    /// The `/bin` records are static, so this is how every field
    /// boundary is reached; the record is untrusted input either way.
    LoadRecord {
        name: Vec<u8>,
        record: Vec<u8>,
    },
    Run {
        name: Vec<u8>,
    },
    Unload {
        name: Vec<u8>,
    },
    State {
        name: Vec<u8>,
    },
    /// Kernel-side lifecycle events an app can reach on its own.
    Exit {
        name: Vec<u8>,
    },
    Fault {
        name: Vec<u8>,
    },
    /// Direct probe of the real mapping-admission API.
    Admit {
        base_va: u64,
        entry: u64,
        text: u64,
        rodata: u64,
        stack: u64,
    },
}

// ---- Fixed boundary bank ------------------------------------------------

pub const FIXED_SCENARIO_NAMES: [&str; 40] = [
    "load_hello_valid",
    "load_counter_valid",
    "load_fault_demo_valid",
    "load_unknown_app",
    "load_empty_name",
    "load_name_27",
    "load_name_28",
    "fixture_bad_magic",
    "fixture_bad_cap",
    "fixture_bad_checksum",
    "magic_wrong_first_byte",
    "magic_wrong_version",
    "magic_no_trailing_space",
    "record_len_12",
    "record_len_13",
    "record_len_64",
    "record_len_65",
    "checksum_wrong",
    "checksum_uppercase_hex",
    "checksum_non_hex",
    "checksum_three_digits",
    "checksum_missing_space",
    "entry_equals_text",
    "entry_past_text",
    "entry_max",
    "text_zero",
    "text_max",
    "text_over_max",
    "rodata_over_max",
    "stack_zero",
    "stack_two",
    "image_mismatch",
    "image_over_max",
    "caps_unknown_word",
    "caps_empty",
    "caps_excessive_for_fault_demo",
    "field_missing",
    "field_extra",
    "double_space",
    "trailing_byte_after_checksum",
];

const MANDATORY_SCENARIOS: u64 = FIXED_SCENARIO_NAMES.len() as u64;

/// Replace one field of a canonical record, recomputing the checksum so
/// only the field under test is wrong.
fn record_with(
    app: usize,
    entry: u64,
    text: u64,
    rodata: u64,
    stack: u64,
    caps: &[u8],
    image: u64,
) -> Vec<u8> {
    build_record(APPS[app].0, entry, text, rodata, stack, caps, image)
}

fn load_record(app: usize, record: Vec<u8>) -> LoaderOp {
    LoaderOp::LoadRecord {
        name: APPS[app].0.to_vec(),
        record,
    }
}

fn mandatory_scenario(index: u64) -> Vec<LoaderOp> {
    let load = |name: &[u8]| {
        vec![LoaderOp::Load {
            name: name.to_vec(),
        }]
    };
    match index {
        0 => load(b"hello"),
        1 => load(b"counter"),
        2 => load(b"fault_demo"),
        3 => load(b"nosuchapp"),
        4 => load(b""),
        5 => load(&[b'a'; NAME_MAX]),
        6 => load(&[b'a'; NAME_MAX + 1]),
        7 => load(b"invalid_bad_magic"),
        8 => load(b"invalid_bad_cap"),
        9 => load(b"invalid_bad_checksum"),
        10 => {
            let mut record = canonical_record(0);
            record[0] = b'B';
            vec![load_record(0, record)]
        }
        11 => {
            let mut record = canonical_record(0);
            record[5] = b'2';
            vec![load_record(0, record)]
        }
        12 => {
            let mut record = canonical_record(0);
            record[6] = b'_';
            vec![load_record(0, record)]
        }
        13 => vec![load_record(0, vec![b'A'; REC_MIN - 1])],
        14 => vec![load_record(0, vec![b'A'; REC_MIN])],
        15 => vec![load_record(0, vec![b'A'; REC_MAX])],
        16 => vec![load_record(0, vec![b'A'; REC_MAX + 1])],
        17 => {
            let mut record = canonical_record(0);
            let last = record.len() - 1;
            record[last] = if record[last] == b'0' { b'1' } else { b'0' };
            vec![load_record(0, record)]
        }
        18 => {
            // Uppercase hex in the checksum tail: parse_hex4 accepts
            // lowercase only.
            let body = b"AXAPP1 hello 0 4096 4096 1 console 8192".to_vec();
            let sum = checksum(&body);
            let mut record = body;
            record.push(b' ');
            record.extend_from_slice(format!("{sum:04X}").as_bytes());
            vec![load_record(0, record)]
        }
        19 => {
            let mut record = canonical_record(0);
            let last = record.len() - 1;
            record[last] = b'z';
            vec![load_record(0, record)]
        }
        20 => {
            let mut record = canonical_record(0);
            record.pop();
            vec![load_record(0, record)]
        }
        21 => {
            let mut record = canonical_record(0);
            let at = record.len() - 5;
            record[at] = b'x';
            vec![load_record(0, record)]
        }
        22 => vec![load_record(
            0,
            record_with(0, 4096, 4096, 4096, 1, CAP_CONSOLE, 8192),
        )],
        23 => vec![load_record(
            0,
            record_with(0, 4097, 4096, 4096, 1, CAP_CONSOLE, 8192),
        )],
        24 => vec![load_record(
            0,
            record_with(0, u64::MAX, 4096, 4096, 1, CAP_CONSOLE, 8192),
        )],
        25 => vec![load_record(
            0,
            record_with(0, 0, 0, 4096, 1, CAP_CONSOLE, 4096),
        )],
        26 => vec![load_record(
            0,
            record_with(0, 0, TEXT_MAX, 0, 1, CAP_CONSOLE, TEXT_MAX),
        )],
        27 => vec![load_record(
            0,
            record_with(0, 0, TEXT_MAX + 1, 0, 1, CAP_CONSOLE, TEXT_MAX + 1),
        )],
        28 => vec![load_record(
            0,
            record_with(
                0,
                0,
                4096,
                RODATA_MAX + 1,
                1,
                CAP_CONSOLE,
                RODATA_MAX + 4097,
            ),
        )],
        29 => vec![load_record(
            0,
            record_with(0, 0, 4096, 4096, 0, CAP_CONSOLE, 8192),
        )],
        30 => vec![load_record(
            0,
            record_with(0, 0, 4096, 4096, 2, CAP_CONSOLE, 8192),
        )],
        31 => vec![load_record(
            0,
            record_with(0, 0, 4096, 4096, 1, CAP_CONSOLE, 9999),
        )],
        32 => vec![load_record(
            0,
            record_with(0, 0, TEXT_MAX, RODATA_MAX, 1, CAP_CONSOLE, IMAGE_MAX + 1),
        )],
        33 => vec![load_record(
            0,
            record_with(0, 0, 4096, 4096, 1, b"mmio", 8192),
        )],
        34 => {
            // Empty caps token collapses two spaces: malformed, not a
            // capability decision.
            let body = b"AXAPP1 hello 0 4096 4096 1  8192".to_vec();
            let sum = checksum(&body);
            let mut record = body;
            record.push(b' ');
            record.extend_from_slice(format!("{sum:04x}").as_bytes());
            vec![load_record(0, record)]
        }
        35 => vec![load_record(
            2,
            record_with(2, 0, 4096, 4096, 1, CAP_CONSOLE, 8192),
        )],
        36 => {
            let body = b"AXAPP1 hello 0 4096 4096 1 console".to_vec();
            let sum = checksum(&body);
            let mut record = body;
            record.push(b' ');
            record.extend_from_slice(format!("{sum:04x}").as_bytes());
            vec![load_record(0, record)]
        }
        37 => {
            let body = b"AXAPP1 hello 0 4096 4096 1 console 8192 extra".to_vec();
            let sum = checksum(&body);
            let mut record = body;
            record.push(b' ');
            record.extend_from_slice(format!("{sum:04x}").as_bytes());
            vec![load_record(0, record)]
        }
        38 => {
            let body = b"AXAPP1 hello  0 4096 4096 1 console 8192".to_vec();
            let sum = checksum(&body);
            let mut record = body;
            record.push(b' ');
            record.extend_from_slice(format!("{sum:04x}").as_bytes());
            vec![load_record(0, record)]
        }
        _ => {
            let mut record = canonical_record(0);
            record.push(b'x');
            vec![load_record(0, record)]
        }
    }
}

/// Lifecycle sequences the roadmap requires, appended to every case so
/// each generated record is also exercised against state transitions.
fn lifecycle_suffix(selector: u8) -> Vec<LoaderOp> {
    let n = |name: &[u8]| name.to_vec();
    match selector % 8 {
        // load -> state -> unload -> load
        0 => vec![
            LoaderOp::Load { name: n(b"hello") },
            LoaderOp::State { name: n(b"hello") },
            LoaderOp::Unload { name: n(b"hello") },
            LoaderOp::Load { name: n(b"hello") },
        ],
        // duplicate load
        1 => vec![
            LoaderOp::Load {
                name: n(b"counter"),
            },
            LoaderOp::Load {
                name: n(b"counter"),
            },
        ],
        // unload when absent
        2 => vec![
            LoaderOp::Unload { name: n(b"hello") },
            LoaderOp::Unload {
                name: n(b"nosuchapp"),
            },
        ],
        // run when not loaded, then properly
        3 => vec![
            LoaderOp::Run { name: n(b"hello") },
            LoaderOp::Load { name: n(b"hello") },
            LoaderOp::Run { name: n(b"hello") },
        ],
        // reload after exit
        4 => vec![
            LoaderOp::Load { name: n(b"hello") },
            LoaderOp::Run { name: n(b"hello") },
            LoaderOp::Exit { name: n(b"hello") },
            LoaderOp::State { name: n(b"hello") },
            LoaderOp::Run { name: n(b"hello") },
        ],
        // reload after fault
        5 => vec![
            LoaderOp::Load {
                name: n(b"fault_demo"),
            },
            LoaderOp::Run {
                name: n(b"fault_demo"),
            },
            LoaderOp::Fault {
                name: n(b"fault_demo"),
            },
            LoaderOp::State {
                name: n(b"fault_demo"),
            },
            LoaderOp::Unload {
                name: n(b"fault_demo"),
            },
        ],
        // invalid load followed by valid load of the same app
        6 => vec![
            load_record(0, record_with(0, 0, 4096, 4096, 2, CAP_CONSOLE, 8192)),
            LoaderOp::State { name: n(b"hello") },
            LoaderOp::Load { name: n(b"hello") },
        ],
        // mapping admission probes around the policy edges
        _ => vec![
            LoaderOp::Admit {
                base_va: USER_BASE_VA,
                entry: 0,
                text: 4096,
                rodata: 4096,
                stack: 1,
            },
            LoaderOp::Admit {
                base_va: kernel::loader::KERNEL_BASE,
                entry: 0,
                text: 4096,
                rodata: 4096,
                stack: 1,
            },
        ],
    }
}

// ---- Deterministic decoding ---------------------------------------------

const NAMES: [&[u8]; 10] = [
    b"hello",
    b"counter",
    b"fault_demo",
    b"invalid_bad_magic",
    b"invalid_bad_cap",
    b"invalid_bad_checksum",
    b"nosuchapp",
    b"HELLO",
    b"hello ",
    b"",
];

const NUMBERS: [u64; 12] = [
    0,
    1,
    4095,
    4096,
    4097,
    TEXT_MAX,
    TEXT_MAX + 1,
    IMAGE_MAX,
    IMAGE_MAX + 1,
    4_294_967_295,
    u64::MAX,
    8192,
];

const CAP_WORDS: [&[u8]; 8] = [
    b"console",
    b"none",
    b"mmio",
    b"dma",
    b"control",
    b"storage",
    b"CONSOLE",
    b"consolex",
];

pub fn decode_operations(iteration: u64, input: &[u8]) -> Vec<LoaderOp> {
    let mut operations = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    operations.extend(lifecycle_suffix((iteration % 251) as u8));
    for chunk in input.chunks(6) {
        if operations.len() >= MAX_LOADER_OPS_PER_CASE {
            break;
        }
        let family = chunk[0];
        let name = NAMES[chunk.get(1).copied().unwrap_or(0) as usize % NAMES.len()];
        let app = (chunk.get(1).copied().unwrap_or(0) as usize) % APPS.len();
        let a = NUMBERS[chunk.get(2).copied().unwrap_or(0) as usize % NUMBERS.len()];
        let b = NUMBERS[chunk.get(3).copied().unwrap_or(0) as usize % NUMBERS.len()];
        let caps = CAP_WORDS[chunk.get(4).copied().unwrap_or(0) as usize % CAP_WORDS.len()];
        let mutator = chunk.get(5).copied().unwrap_or(family);

        let operation = match family % 10 {
            0 => LoaderOp::Load {
                name: name.to_vec(),
            },
            1 => LoaderOp::Run {
                name: name.to_vec(),
            },
            2 => LoaderOp::Unload {
                name: name.to_vec(),
            },
            3 => LoaderOp::State {
                name: name.to_vec(),
            },
            4 => LoaderOp::Exit {
                name: name.to_vec(),
            },
            5 => LoaderOp::Fault {
                name: name.to_vec(),
            },
            6 => LoaderOp::Admit {
                base_va: if mutator & 1 == 0 {
                    USER_BASE_VA
                } else {
                    kernel::loader::KERNEL_BASE - a.min(8192)
                },
                entry: a,
                text: b,
                rodata: a,
                stack: u64::from(mutator % 3),
            },
            // Synthesized records: a well-formed one with fuzzed fields,
            // then byte-level mutations on top.
            7 => load_record(app, record_with(app, a, b, a, 1, caps, b.wrapping_add(a))),
            8 => load_record(
                app,
                record_with(app, a, b, a, u64::from(mutator % 3), caps, b),
            ),
            _ => {
                let mut record = record_with(app, 0, 4096, 4096, 1, caps, 8192);
                match mutator % 5 {
                    0 => {}
                    1 => {
                        let position = mutator as usize % record.len();
                        record[position] ^= 0x20;
                    }
                    2 => record.truncate(mutator as usize % (record.len() + 1)),
                    3 => {
                        let position = mutator as usize % (record.len() + 1);
                        record.insert(position, b' ');
                    }
                    _ => record.push(mutator),
                }
                load_record(app, record)
            }
        };
        operations.push(operation);
    }
    operations.truncate(MAX_LOADER_OPS_PER_CASE);
    operations
}

// ---- Fuzz target ---------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct LoaderTarget;

impl FuzzTarget for LoaderTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure(
                "loader target received a case for another target",
            );
        }
        evaluate_operations(&decode_operations(case.iteration, &case.input))
    }
}

pub fn evaluate_operations(operations: &[LoaderOp]) -> CaseResult {
    if operations.len() > MAX_LOADER_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "loader operation count {} exceeds bound {MAX_LOADER_OPS_PER_CASE}",
            operations.len()
        ));
    }
    let first = match run_caught(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    let second = match run_caught(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    if first != second {
        return CaseResult::invariant_failure(
            "LD-INV-008: identical loader streams produced different final states",
        );
    }
    CaseResult::safe_reject("loader sequence completed without an invariant failure")
}

fn run_caught(operations: &[LoaderOp]) -> Result<(Protected, Vec<Vec<u8>>), String> {
    match catch_unwind(AssertUnwindSafe(|| run_once(operations))) {
        Ok(result) => result,
        Err(_) => Err(
            "LD-INV-009: user-controlled loader input triggered a host-reachable panic".to_string(),
        ),
    }
}

fn run_once(operations: &[LoaderOp]) -> Result<(Protected, Vec<Vec<u8>>), String> {
    let mut model = LoaderModel::new();
    for operation in operations {
        model.apply(operation)?;
    }
    Ok((model.protected.clone(), model.replies.clone()))
}

impl LoaderModel {
    fn new() -> Self {
        Self {
            protected: Protected {
                states: [AppState::Available; 3],
                tasks: [TaskState::NeverRun; 3],
                granted: [None; 3],
                starts: [0; 3],
            },
            replies: Vec::new(),
        }
    }

    /// Fetch a `/bin` record through the fs model (docs/33 §7). Mirrors
    /// `ld_fetch`: an empty or over-long name never leaves the loader,
    /// and any `ERR` reply is a fetch failure.
    fn fetch(name: &[u8]) -> Option<Vec<u8>> {
        if name.is_empty() || name.len() > NAME_MAX {
            return None;
        }
        let mut request = b"CAT /bin/".to_vec();
        request.extend_from_slice(name);
        request.extend_from_slice(b".app");
        match fs::fs_reply(&request, true) {
            fs::Outcome::Reply(reply) if !reply.starts_with(b"ERR") => Some(reply),
            _ => None,
        }
    }

    fn apply(&mut self, operation: &LoaderOp) -> Result<(), String> {
        let before = self.protected.clone();
        match operation {
            LoaderOp::Load { name } => {
                let record = Self::fetch(name);
                self.load(name, record, &before)?;
            }
            LoaderOp::LoadRecord { name, record } => {
                // A record longer than the transport never reaches the
                // validator: ld_fetch's recv is bounded at 64 bytes.
                let fetched = if record.len() > REC_MAX {
                    None
                } else {
                    Some(record.clone())
                };
                self.load(name, fetched, &before)?;
            }
            LoaderOp::Run { name } => self.run(name, &before)?,
            LoaderOp::Unload { name } => self.unload(name, &before)?,
            LoaderOp::State { name } => self.state(name, &before)?,
            LoaderOp::Exit { name } => {
                if let Some(id) = app_id(name) {
                    if self.protected.tasks[id] == TaskState::Running {
                        self.protected.tasks[id] = TaskState::Exited;
                    }
                }
            }
            LoaderOp::Fault { name } => {
                if let Some(id) = app_id(name) {
                    if self.protected.tasks[id] == TaskState::Running {
                        self.protected.tasks[id] = TaskState::Faulted;
                    }
                }
            }
            LoaderOp::Admit {
                base_va,
                entry,
                text,
                rodata,
                stack,
            } => {
                let layout = ImageLayout {
                    base_va: *base_va,
                    entry_offset: *entry,
                    text_size: *text,
                    rodata_size: *rodata,
                    stack_pages: *stack,
                };
                // LD-INV-005: the real kernel admission API must never
                // panic and must never admit a kernel-space or W^X-
                // violating layout. It also must not mutate loader state.
                let verdict = admit_image_mapping(&layout);
                if verdict.is_ok() && (*base_va >= kernel::loader::KERNEL_BASE || *stack != 1) {
                    return Err(format!("LD-INV-005: admitted an illegal layout {layout:?}"));
                }
                if self.protected != before {
                    return Err("LD-INV-003: a mapping probe mutated loader state".to_string());
                }
            }
        }
        Ok(())
    }

    fn load(
        &mut self,
        name: &[u8],
        record: Option<Vec<u8>>,
        before: &Protected,
    ) -> Result<(), String> {
        let id = app_id(name);
        // Already loaded: deterministic refusal, no state change.
        if let Some(id) = id {
            if self.protected.states[id] != AppState::Available {
                self.replies.push(b"ERR already_loaded".to_vec());
                return self.require_unchanged(before, "LD-INV-007", "duplicate load");
            }
        }
        let Some(record) = record else {
            self.replies.push(Verdict::NotFound.reply().to_vec());
            return self.require_unchanged(before, "LD-INV-003", "failed fetch");
        };

        let verdict = validate(&record, name);
        let expected = expected_verdict(&record, name);
        if verdict != expected {
            return Err(format!(
                "LD-INV-002: verdict mismatch for {:?} (model {verdict:?}, oracle {expected:?})",
                String::from_utf8_lossy(&record)
            ));
        }

        if verdict != Verdict::Valid {
            self.replies.push(verdict.reply().to_vec());
            // LD-INV-003 / LD-INV-004: a rejected record installs nothing
            // and grants nothing.
            return self.require_unchanged(before, "LD-INV-003/004", "rejected record");
        }

        // A valid verdict implies a known app (docs/32 §6 rule 7).
        let Some(id) = id else {
            return Err("LD-INV-010: an unknown app name produced a valid verdict".to_string());
        };

        // LD-INV-005: whatever the validator accepted must describe a
        // layout the real kernel mapping mechanism would admit.
        let (entry, text, rodata, stack) = layout_fields(&record);
        let layout = ImageLayout {
            base_va: USER_BASE_VA,
            entry_offset: entry,
            text_size: text,
            rodata_size: rodata,
            stack_pages: stack,
        };
        if let Err(reject) = admit_image_mapping(&layout) {
            return Err(format!(
                "LD-INV-005: accepted record {:?} describes a layout the kernel would reject \
                 ({reject:?})",
                String::from_utf8_lossy(&record)
            ));
        }

        // LD-INV-004: the granted capability must be within policy.
        let caps = caps_field(&record);
        if caps == CAP_CONSOLE && !APPS[id].1 {
            return Err(format!(
                "LD-INV-004: {} was granted console outside its policy",
                String::from_utf8_lossy(APPS[id].0)
            ));
        }
        self.protected.states[id] = AppState::Loaded;
        self.protected.granted[id] = if caps == CAP_CONSOLE {
            Some(CAP_CONSOLE)
        } else {
            Some(CAP_NONE)
        };
        let mut reply = b"OK loaded ".to_vec();
        reply.extend_from_slice(name);
        self.replies.push(reply);
        Ok(())
    }

    fn run(&mut self, name: &[u8], before: &Protected) -> Result<(), String> {
        let Some(id) = app_id(name) else {
            self.replies.push(Verdict::NotFound.reply().to_vec());
            return self.require_unchanged(before, "LD-INV-007", "run of unknown app");
        };
        if self.protected.states[id] == AppState::Available {
            self.replies.push(b"ERR not_loaded".to_vec());
            return self.require_unchanged(before, "LD-INV-007", "run before load");
        }
        self.protected.states[id] = AppState::Running;
        self.protected.tasks[id] = TaskState::Running;
        self.protected.starts[id] += 1;
        let mut reply = b"OK running ".to_vec();
        reply.extend_from_slice(name);
        self.replies.push(reply);
        Ok(())
    }

    fn unload(&mut self, name: &[u8], before: &Protected) -> Result<(), String> {
        let Some(id) = app_id(name) else {
            self.replies.push(Verdict::NotFound.reply().to_vec());
            return self.require_unchanged(before, "LD-INV-007", "unload of unknown app");
        };
        if self.protected.states[id] == AppState::Available {
            self.replies.push(b"ERR not_loaded".to_vec());
            return self.require_unchanged(before, "LD-INV-007", "unload when absent");
        }
        self.protected.states[id] = AppState::Available;
        // Unload drops the grant: authority never outlives the load.
        self.protected.granted[id] = None;
        let mut reply = b"OK unloaded ".to_vec();
        reply.extend_from_slice(name);
        self.replies.push(reply);
        Ok(())
    }

    fn state(&mut self, name: &[u8], before: &Protected) -> Result<(), String> {
        let Some(id) = app_id(name) else {
            self.replies.push(Verdict::NotFound.reply().to_vec());
            return self.require_unchanged(before, "LD-INV-007", "state of unknown app");
        };
        let reply: &[u8] = match self.protected.states[id] {
            AppState::Available => b"state=available",
            AppState::Loaded => b"state=loaded",
            AppState::Running => match self.protected.tasks[id] {
                TaskState::Exited => b"state=exited",
                TaskState::Faulted => b"state=faulted",
                _ => b"state=running",
            },
        };
        self.replies.push(reply.to_vec());
        // Introspection is read-only.
        self.require_unchanged(before, "LD-INV-003", "state query")
    }

    fn require_unchanged(
        &self,
        before: &Protected,
        invariant: &str,
        what: &str,
    ) -> Result<(), String> {
        if self.protected != *before {
            return Err(format!("{invariant}: {what} mutated loader state"));
        }
        Ok(())
    }
}

/// Extract the four layout numbers from a record already known valid.
fn layout_fields(record: &[u8]) -> (u64, u64, u64, u64) {
    let body = &record[MAGIC.len()..record.len() - 5];
    let fields: Vec<&[u8]> = body.split(|byte| *byte == b' ').collect();
    let value = |index: usize| decimal(fields[index]).unwrap_or(0);
    (value(1), value(2), value(3), value(4))
}

/// Capability token of a record already known valid.
fn caps_field(record: &[u8]) -> &[u8] {
    let body = &record[MAGIC.len()..record.len() - 5];
    let start = MAGIC.len();
    let fields: Vec<(usize, usize)> = {
        let mut spans = Vec::new();
        let mut begin = 0usize;
        for (index, byte) in body.iter().enumerate() {
            if *byte == b' ' {
                spans.push((begin, index));
                begin = index + 1;
            }
        }
        spans.push((begin, body.len()));
        spans
    };
    let (from, to) = fields[5];
    &record[start + from..start + to]
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResultClass;
    use kernel::loader::MapReject;

    /// The builder must reproduce the runtime's storage blocks 4-6 byte
    /// for byte, including their checksums — this ties the model to the
    /// real records rather than to my arithmetic.
    #[test]
    fn canonical_records_match_the_runtime_storage_blocks() {
        assert_eq!(
            canonical_record(0),
            b"AXAPP1 hello 0 4096 4096 1 console 8192 0a6d".to_vec()
        );
        assert_eq!(
            canonical_record(1),
            b"AXAPP1 counter 0 4096 4096 1 console 8192 0b59".to_vec()
        );
        assert_eq!(
            canonical_record(2),
            b"AXAPP1 fault_demo 0 4096 4096 1 none 8192 0b36".to_vec()
        );
        // And they are exactly the fs model's /bin records.
        for (app, block) in [(0usize, 4usize), (1, 5), (2, 6)] {
            assert_eq!(
                canonical_record(app),
                crate::targets::storage::BLOCKS[block].to_vec(),
                "app {app} vs storage block {block}"
            );
        }
    }

    #[test]
    fn valid_records_validate_and_map() {
        for (app, (name, _)) in APPS.iter().enumerate() {
            let record = canonical_record(app);
            assert_eq!(validate(&record, name), Verdict::Valid);
            assert_eq!(expected_verdict(&record, name), Verdict::Valid);
            let (entry, text, rodata, stack) = layout_fields(&record);
            assert_eq!(
                admit_image_mapping(&ImageLayout {
                    base_va: USER_BASE_VA,
                    entry_offset: entry,
                    text_size: text,
                    rodata_size: rodata,
                    stack_pages: stack,
                }),
                Ok(())
            );
        }
    }

    #[test]
    fn the_three_static_fixtures_are_rejected_with_their_documented_reasons() {
        // Verbatim from the fs /bin fixtures (docs/33 §3).
        let bad_magic = b"BXAPP1 invalid_bad_magic 0 4096 4096 1 none 8192 0000";
        assert_eq!(validate(bad_magic, b"invalid_bad_magic"), Verdict::BadImage);
        let bad_cap = b"AXAPP1 invalid_bad_cap 0 4096 4096 1 mmio 8192 0d18";
        assert_eq!(
            validate(bad_cap, b"invalid_bad_cap"),
            Verdict::DeniedCapability
        );
        let bad_sum = b"AXAPP1 invalid_bad_checksum 0 4096 4096 1 none 8192 0000";
        assert_eq!(
            validate(bad_sum, b"invalid_bad_checksum"),
            Verdict::BadChecksum
        );
    }

    /// (entry, text, rodata, stack, caps, image, expected verdict).
    type FieldCase = (u64, u64, u64, u64, &'static [u8], u64, Verdict);

    #[test]
    fn every_field_boundary_is_rejected() {
        let cases: [FieldCase; 12] = [
            (0, 4096, 4096, 1, CAP_CONSOLE, 8192, Verdict::Valid),
            (4095, 4096, 4096, 1, CAP_CONSOLE, 8192, Verdict::Valid),
            (4096, 4096, 4096, 1, CAP_CONSOLE, 8192, Verdict::BadImage),
            (0, 0, 4096, 1, CAP_CONSOLE, 4096, Verdict::BadImage),
            (0, TEXT_MAX, 0, 1, CAP_CONSOLE, TEXT_MAX, Verdict::Valid),
            (
                0,
                TEXT_MAX + 1,
                0,
                1,
                CAP_CONSOLE,
                TEXT_MAX + 1,
                Verdict::BadImage,
            ),
            (
                0,
                4096,
                RODATA_MAX + 1,
                1,
                CAP_CONSOLE,
                RODATA_MAX + 4097,
                Verdict::BadImage,
            ),
            (0, 4096, 4096, 0, CAP_CONSOLE, 8192, Verdict::BadImage),
            (0, 4096, 4096, 2, CAP_CONSOLE, 8192, Verdict::BadImage),
            (0, 4096, 4096, 1, CAP_CONSOLE, 9999, Verdict::BadImage),
            (0, 4096, 4096, 1, b"mmio", 8192, Verdict::DeniedCapability),
            (
                0,
                4096,
                4096,
                1,
                b"CONSOLE",
                8192,
                Verdict::DeniedCapability,
            ),
        ];
        for (entry, text, rodata, stack, caps, image, want) in cases {
            let record = build_record(b"hello", entry, text, rodata, stack, caps, image);
            assert_eq!(
                validate(&record, b"hello"),
                want,
                "record {:?}",
                String::from_utf8_lossy(&record)
            );
            assert_eq!(
                expected_verdict(&record, b"hello"),
                want,
                "oracle disagreed on {:?}",
                String::from_utf8_lossy(&record)
            );
        }
    }

    #[test]
    fn fault_demo_may_not_request_console() {
        let excessive = build_record(b"fault_demo", 0, 4096, 4096, 1, CAP_CONSOLE, 8192);
        assert_eq!(
            validate(&excessive, b"fault_demo"),
            Verdict::DeniedCapability
        );
        let allowed = build_record(b"fault_demo", 0, 4096, 4096, 1, CAP_NONE, 8192);
        assert_eq!(validate(&allowed, b"fault_demo"), Verdict::Valid);
    }

    #[test]
    fn checksum_cannot_be_bypassed() {
        let mut record = canonical_record(0);
        let last = record.len() - 1;
        record[last] = if record[last] == b'0' { b'1' } else { b'0' };
        assert_eq!(validate(&record, b"hello"), Verdict::BadChecksum);
        // Uppercase hex is not accepted by parse_hex4.
        let body = b"AXAPP1 hello 0 4096 4096 1 console 8192".to_vec();
        let sum = checksum(&body);
        let mut upper = body.clone();
        upper.push(b' ');
        upper.extend_from_slice(format!("{sum:04X}").as_bytes());
        assert_eq!(validate(&upper, b"hello"), Verdict::Malformed);
        // A record whose checksum is right but whose fields are junk is
        // still malformed — the checksum protects integrity, not grammar.
        let mut junk = b"AXAPP1 hello x y z w console 8192".to_vec();
        let sum = checksum(&junk);
        junk.push(b' ');
        junk.extend_from_slice(format!("{sum:04x}").as_bytes());
        assert_eq!(validate(&junk, b"hello"), Verdict::Malformed);
    }

    #[test]
    fn structural_boundaries() {
        // Shortest possible record is malformed, not a panic.
        assert_eq!(validate(&[b'A'; REC_MIN - 1], b"hello"), Verdict::Malformed);
        assert_eq!(validate(&[b'A'; REC_MIN], b"hello"), Verdict::BadImage);
        assert_eq!(validate(b"", b"hello"), Verdict::Malformed);
        // Trailing bytes after the checksum move the tail, so the record
        // no longer ends where the image field says it does.
        let mut trailing = canonical_record(0);
        trailing.push(b'x');
        assert_ne!(validate(&trailing, b"hello"), Verdict::Valid);
        // A name that does not match the request is a wrong image.
        assert_eq!(
            validate(&canonical_record(0), b"counter"),
            Verdict::BadImage
        );
    }

    #[test]
    fn name_length_bounds_are_enforced_before_fetch() {
        assert!(LoaderModel::fetch(b"").is_none());
        assert!(LoaderModel::fetch(&[b'a'; NAME_MAX + 1]).is_none());
        assert!(LoaderModel::fetch(b"hello").is_some());
        assert!(LoaderModel::fetch(b"nosuchapp").is_none());
    }

    fn run(operations: &[LoaderOp]) -> (Protected, Vec<Vec<u8>>) {
        run_once(operations).expect("model run must not fail an invariant")
    }

    #[test]
    fn rejected_load_installs_nothing() {
        let baseline = run(&[]);
        for record in [
            build_record(b"hello", 0, 4096, 4096, 2, CAP_CONSOLE, 8192),
            build_record(b"hello", 4096, 4096, 4096, 1, CAP_CONSOLE, 8192),
            build_record(b"hello", 0, 4096, 4096, 1, b"mmio", 8192),
            vec![b'A'; REC_MAX + 1],
        ] {
            let (state, _) = run(&[load_record(0, record)]);
            assert_eq!(state, baseline.0, "a rejected record changed loader state");
        }
    }

    #[test]
    fn lifecycle_sequences_are_legal_and_deterministic() {
        for selector in 0..8u8 {
            let operations = lifecycle_suffix(selector);
            let result = evaluate_operations(&operations);
            assert_ne!(
                result.class,
                ResultClass::KernelInvariantFailure,
                "lifecycle {selector}: {}",
                result.reason
            );
        }
        // load -> state -> unload -> load leaves the app loaded again.
        let (state, replies) = run(&lifecycle_suffix(0));
        assert_eq!(state.states[0], AppState::Loaded);
        assert_eq!(replies[1], b"state=loaded".to_vec());
        // Duplicate load is refused deterministically.
        let (_, replies) = run(&lifecycle_suffix(1));
        assert_eq!(replies[1], b"ERR already_loaded".to_vec());
        // Unload when absent is refused.
        let (_, replies) = run(&lifecycle_suffix(2));
        assert_eq!(replies[0], b"ERR not_loaded".to_vec());
        // Run before load is refused, then succeeds.
        let (_, replies) = run(&lifecycle_suffix(3));
        assert_eq!(replies[0], b"ERR not_loaded".to_vec());
        assert_eq!(replies[2], b"OK running hello".to_vec());
        // Exited app stays loaded and is visible as exited, then re-runs.
        // `Exit` is a kernel-side event, not a loader command, so it
        // contributes no reply: the state answer is the third one.
        let (state, replies) = run(&lifecycle_suffix(4));
        assert_eq!(replies[2], b"state=exited".to_vec());
        assert_eq!(state.starts[0], 2, "an exited app can be re-run");
    }

    #[test]
    fn unload_drops_the_capability_grant() {
        let (state, _) = run(&[
            LoaderOp::Load {
                name: b"hello".to_vec(),
            },
            LoaderOp::Unload {
                name: b"hello".to_vec(),
            },
        ]);
        assert_eq!(state.granted[0], None);
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
            assert!(!operations.is_empty(), "scenario {index} is empty");
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
    fn every_decoded_stream_is_invariant_clean() {
        for seed in 0..=u8::MAX {
            let input = [seed, seed.wrapping_mul(5), 200, seed ^ 0x3c, 7, seed / 3];
            let operations = decode_operations(u64::from(seed), &input);
            assert!(operations.len() <= MAX_LOADER_OPS_PER_CASE);
            let result = evaluate_operations(&operations);
            assert_ne!(
                result.class,
                ResultClass::KernelInvariantFailure,
                "byte {seed}: {}",
                result.reason
            );
        }
    }

    #[test]
    fn decoding_is_deterministic() {
        let input = [3, 1, 5, 9, 2, 6, 5, 3, 5];
        assert_eq!(decode_operations(11, &input), decode_operations(11, &input));
    }

    #[test]
    fn mapping_admission_rejects_illegal_layouts() {
        // Kernel-space target, oversized image, bad entry, stack policy.
        assert_eq!(
            admit_image_mapping(&ImageLayout {
                base_va: kernel::loader::KERNEL_BASE,
                entry_offset: 0,
                text_size: 4096,
                rodata_size: 0,
                stack_pages: 1,
            }),
            Err(MapReject::KernelAddress)
        );
        assert_eq!(
            admit_image_mapping(&ImageLayout {
                base_va: USER_BASE_VA,
                entry_offset: 0,
                text_size: TEXT_MAX,
                rodata_size: RODATA_MAX + 1,
                stack_pages: 1,
            }),
            Err(MapReject::Oversized)
        );
    }
}
