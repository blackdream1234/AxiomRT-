# AxiomRT v1.6 Evidence Report — Storage-Backed Restricted Loader

Phase: AXIOM-LOAD (spec: `AxiomRT v1.6.md`; design: docs/32, docs/33).
Tag: `v1.6-storage-backed-loader`.
Archived: 2026-07-08. Tool versions: `tool_versions.txt`.

## 1. What v1.6 demonstrates

The first restricted application-loading path on AxiomRT, QEMU-verified
end to end (`restricted_loader_qemu_test.log`, 30 assertions;
`verify_all.log`, 15/15 QEMU tests + host suites + Coq):

* A **restricted app image format** (`AXAPP1`, docs/32): one bounded
  record — magic, version, name, entry_offset, text_size, rodata_size,
  stack_pages, required_capabilities, image_size, checksum.
* A **`/bin` read-only tree** (docs/33 §3): `ls /bin` lists six `.app`
  records; for the three valid apps the record is **storage-backed** —
  `fs_service` fetches it from `storage_service` blocks 4–6 over its
  storage capability (the same nested fs→storage IPC chain as
  `/storage/version`), strips the `OK data=` frame, and forwards the
  bare record. Three invalid records are fs-static test fixtures.
* A **user-space loader validator** (docs/32 §6): fixed-order checks —
  magic/version, checksum (16-bit additive), layout (image = text +
  rodata, bounded, entry inside text, stack policy, W^X by region
  separation), capability request (`console|none` vocabulary, per-app
  policy), and known-app. Each failure maps to a distinct bounded
  error.
* A **loaded-app state machine** (docs/32, docs/33 §7):
  available→loaded→running, with exited/faulted resolved through a new
  read-only kernel introspection (sys_info kind 7); unload returns to
  available; repeated load/run is deterministic.
* **Shell commands**: `bin`, `app load <name>`, `app unload <name>`,
  `app state <name>`, `run loaded <name>`; the legacy `apps` /
  `app info <name>` / `run <name>` are unchanged.
* **Safe outcomes** proven in one session: load+run hello (from the
  storage-backed record) and counter; `invalid_bad_magic` →
  `ERR bad_image`, `invalid_bad_checksum` → `ERR bad_checksum`,
  `invalid_bad_cap` → `ERR denied_capability` (each with an
  `APP_IMAGE rejected=` evidence line, no kernel fault); load+run
  `fault_demo` is `CAP_DENIED` then watchdog-contained and
  supervisor-killed, with the shell alive throughout.

No existing behavior regressed: all 14 prior QEMU tests pass; the
legacy static `run hello`, `ls`, `storage info`, and `drivers` still
work after loader activity; the kernel parses no names, paths, or
image bytes.

## 2. What remains static

* The `/bin` records and the storage block image are build-time
  static. "Storage-backed" means the record bytes travel the real
  fs→storage service chain, not that they are external or persistent.
* v1.6 records **reference** code that is statically present in the
  kernel image's `.user` region (docs/32 §3): the record describes and
  validates a mapping envelope; no code bytes are loaded into fresh
  pages. The loader validates a real image header exactly as it would a
  real image; swapping in real bytes changes the fetch step, not the
  validation, capability, state-machine, or shell layers.

## 3. What is not full ELF

No ELF parsing, no program headers, no relocation, no dynamic linking.
`AXAPP1` is a deliberately minimal fixed-field record so every
validation rule is total and host-testable (docs/32 §1). The future
ELF path (docs/32 §10) replaces the fetch/format without changing the
policy path introduced here.

## 4. What is not writable storage

The filesystem and storage remain read-only. There is no `WRITE`
protocol, no journaling, no quota. `/bin` records cannot be modified at
runtime — a safety property and a functional limitation at once
(docs/33 §8/§9).

## 5. What is not production persistence

Nothing in v1.6 persists across a reboot; the block image is a static
`.user.rodata` array. The block-backed persistence path (client → fs →
storage → driver → real virtio-blk) is assembled but not delivered
(docs/33 §10).

## 6. No certification claim

Nothing in v1.6 is DO-178C/ISO 26262/IEC 61508 evidence; this is an
emulator-only evaluation build. The checksum detects corruption, not
tampering — no signing (docs/32 §9).

## 7. Next phase

`v1.7-minimal-network-service`. Still no writable storage, no arbitrary
ELF, no real hardware. The virtio-blk driver (docs/30 §5, docs/31 §15)
remains the path that turns the storage-backed loader into real
persistence.

## Notable engineering note

A third `.user`-region `.rodata` lookup-table escape was found and
fixed in this phase (AXIOM-LOAD-004 and -008): growing a same-callee
constant-arg branch chain past a threshold, and mixing a dense integer
switch's success and reject arms in one function, both made LLVM hoist
constants/dispatch into a kernel-`.rodata` table that U-mode cannot
reach (contained page faults on 0xf020 and 0xf000). Both were resolved
by the established pattern: uniform branch arms, ≤5 per function, in
their own function (docs/25 §2).

## CI limitation

GitHub Actions workflows exist in-repo (`.github/workflows/`), but the
Actions gate on this push could not be confirmed from the local
environment at archive time; the authoritative evidence is the local
sweep in `verify_all.log` (VERIFY ALL: PASS, 15/15).
