# 32 — Restricted App Image Format (AXAPP1)

Document ID: created by AXIOM-LOAD-001 (Phase v1.6).
Requirement reference: `AxiomRT v1.6.md`, docs/27, docs/31.

## 1. Why full ELF is out of scope for v1.6

ELF loading means relocation, section parsing, symbol handling, and a
large attack surface fed by untrusted bytes — exactly the code that
compromises loaders in production systems. AxiomRT's v1.6 goal is the
*policy path* (fetch → validate → grant → run → contain), not the
binary-format engineering. A restricted, fixed-field, bounded record
lets every validation rule be total and testable now; ELF (§10) can
replace the record format later without changing the policy path.

## 2. Restricted app image goals

* One bounded record (≤ 64 bytes, one IPC reply; ≤ 48 bytes when
  storage-block-backed) fully describes an app image.
* Every field is explicit; nothing is inferred.
* Validation is total: any malformed input yields a bounded error,
  never a crash and never a partial load.
* Capability requests are declared in the image and checked against
  per-app policy — no ambient authority.
* W^X by construction: text and rodata are described (and mapped)
  separately; no writable+executable combination is representable.

## 3. Image fields

Wire form (single line, space-separated, positional):

```text
AXAPP1 <name> <entry_offset> <text_size> <rodata_size> <stack_pages> <caps> <image_size> <checksum>
```

| Field | Wire position | Meaning |
|---|---|---|
| magic | 1 (chars 0–4, `AXAPP`) | format identifier |
| version | 1 (char 5, `1`) | format version, this document |
| name | 2 | app name (lowercase, `_`, ≤ 27 chars) |
| entry_offset | 3 | entry point offset inside text |
| text_size | 4 | executable region size (bytes) |
| rodata_size | 5 | read-only data region size (bytes) |
| stack_pages | 6 | private stack pages requested |
| required_capabilities | 7 | `console` or `none` (v1.6 vocabulary) |
| image_size | 8 | total image size; must equal text+rodata |
| checksum | 9 | 4 lowercase hex digits (§6) |

v1.6 truth (stated, not hidden): the record *references* code that is
statically present in the kernel image's `.user` region (docs/25 §2).
No code bytes travel through IPC and nothing is copied into fresh
pages; `text_size`/`rodata_size`/`image_size` describe the **validated
mapping envelope** the app runs under, not a measured per-function
size. The loader validates the record exactly as it would validate a
real image header; swapping in real image bytes later changes the
fetch step, not the validation or grant steps.

## 4. Manifest fields

The existing human-readable manifests (`/apps/<name>.manifest`,
docs/27 §4) are unchanged: name, purpose, priority, capability list,
restart policy. The manifest is the operator-facing description; the
`.app` record is the loader-facing one. v1.6 keeps both and they must
agree on the capability list (the loader's per-app policy table is the
arbiter).

## 5. Capability request model

* The record's `caps` field is the app's complete request; v1.6
  vocabulary is exactly `console` (console write) or `none`.
* The loader holds a per-app policy table (name → allowed caps). A
  request is granted only if it is a subset of policy:
  `hello: console`, `counter: console`, `fault_demo: none`.
* Any unknown capability word (`mmio`, `dma`, `irq`, `control`,
  `storage`, …) → `ERR denied_capability`. Apps can never request
  device, driver, control, or storage authority (docs/31 §10; v1.6
  forbidden list).
* An excessive request (known word, not in that app's policy) →
  `ERR denied_capability`.

## 6. Loader validation rules

Checks run in fixed order; the first failure answers and stops:

1. record length ≥ minimum and ≤ 64 bytes (transport already bounds it);
2. magic is `AXAPP`, version is `1` → else `ERR bad_image`;
3. all nine fields present and numeric fields parse → else
   `ERR malformed`;
4. checksum: 16-bit additive sum of every record byte except the
   trailing 5 (` ` + 4 hex digits), rendered as 4 lowercase hex
   digits, must equal the checksum field → else `ERR bad_checksum`;
5. layout: `image_size == text_size + rodata_size`, sizes within
   bounds (text ≤ 65536, rodata ≤ 65536, image ≤ 131072, text > 0),
   `entry_offset < text_size`, `stack_pages == 1` (v1.6 policy) →
   else `ERR bad_image`;
6. capability request known and within the app's policy → else
   `ERR denied_capability`;
7. name refers to a known static app (v1.6: `hello`, `counter`,
   `fault_demo`) and matches the requested name → else
   `ERR not_found`.

Only full success moves the app to Loaded (docs/33; state machine in
`app_loader_service`).

## 7. Kernel boundary

The kernel provides mechanisms only: address-space construction with
W^X permissions (text U+RX, rodata U+R, stack U+RW — docs/12 §5,
docs/25 §2), task registration/start/kill/restart, capability minting
from the validated table entry, scheduling, IPC, fault containment,
and read-only task-state introspection. The kernel never sees the
record: it does not parse app names, paths, manifests, or image
fields, and it has no filesystem or selection policy. Image policy
lives entirely in `app_loader_service`.

### 7.1 Mapping mechanism review (AXIOM-LOAD-009)

v1.6 adds **no** new kernel image-mapping syscall. A restricted image
is represented by the existing static service/app mapping mechanism,
`paging_hw::build_service_address_space`, which already maps validated
text as U+R+X, rodata as U+R, and the stack as U+R+W, and the page
table's permission check (`validate_perms`, MEM-P5) already rejects any
W+X user mapping, any user device mapping, and any kernel-frame user
mapping. The loader's own validator (§6) rejects oversized images, bad
entry offsets, and out-of-policy stack requests in user space before
any run.

The admission rules such a mapping must satisfy are modeled and host-
tested in `kernel::loader` (`admit_image_mapping`): W^X rejection
(exercised through the real `PageTable::map` path), kernel-address
rejection, oversized-image rejection, and bad-entry rejection. This
documents the boundary a future map-validated-image mechanism (or the
ELF path, §10) would enforce, without adding kernel attack surface in
v1.6.

## 8. Filesystem/storage relationship

Records are filesystem objects under `/bin` (docs/33 §3):
`ls /bin` lists them; `CAT /bin/<name>.app` returns the record. For
the three valid apps the record content is **storage-backed**:
fs_service fetches it from storage_service blocks 4–6 over its
storage capability (nested bounded IPC, same path as
`/storage/version`, docs/29 §7). The invalid test records are
fs-static. The loader only ever speaks to fs_service.

## 9. Security limitations

* No cryptographic integrity: the checksum detects corruption, not
  tampering. Signing is future work.
* The code behind a record is build-time trusted (it is compiled into
  the kernel image); v1.6 does not prove "loaded bytes == validated
  bytes" because no bytes are loaded.
* The loader is itself a U-mode service: a loader compromise can
  start known apps with their policy caps, but cannot mint authority
  beyond its own capabilities (no device, fs-read + app-start only).
* Names, sizes, and caps are validated; timing/resource exhaustion by
  repeated load/unload is bounded only by the shell's serial rate.
* No writable storage: records cannot be modified at runtime, which
  is a safety property and a functional limitation at once.

## 10. Future ELF path

A future phase replaces the record fetch with: fetch ELF header +
program headers (bounded reads through fs/storage), validate machine/
class/flags, load PT_LOAD segments into fresh frames via a kernel
map-validated-region mechanism (docs/31-style grant: U+RX for code,
U+R for rodata, U+RW for data/stack, W^X denied at the mechanism),
no relocation until PIE support is explicitly designed, checksum
replaced by a signature. The validation order, capability policy,
state machine, and shell commands introduced in v1.6 are unchanged by
that swap.
