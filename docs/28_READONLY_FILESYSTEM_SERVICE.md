# 28 — Read-only Filesystem Service

Document ID: created by AXIOM-FS-001.
Requirement reference: `axiomFX.md`, docs/25_OS_BOOT_FLOW.md,
docs/26_SHELL.md, docs/27_APPLICATION_MODEL.md.

## 1. Why the filesystem is user-space

The Architecture Law forbids filesystem logic in the kernel. Path
parsing, name lookup, and content policy are exactly the kind of
complex, fault-prone code the microkernel exists to isolate: a
filesystem bug must be a contained user fault, never a kernel fault.
The kernel contributes only its existing mechanisms — tasks, address
spaces, bounded IPC, capabilities, scheduling, containment. `fs_service`
never sees kernel-only memory (hardware-enforced, as for every service).

## 2. Embedded read-only stage vs future storage-backed stage

This stage: the filesystem *image* is a fixed set of sectioned
constants in the service's `.user.rodata` (docs/25 §2 constrained
rules) — no storage, no block devices, no writes, no dynamic app
loading. The future storage stage replaces the static table with
blocks fetched from `storage_service` (virtio-blk first) behind the
**same IPC protocol**, so the shell and clients do not change.

## 3. IPC protocol (AXIOM-FS-004)

Transport: the existing synchronous bounded IPC (≤ 64 bytes per
message) on endpoint 4 (`EP_FS`). One request → one reply
(single-chunk in this stage; multi-chunk continuation is reserved for
the storage stage and `ERR too_long` is the guard).

Requests (ASCII): `LS <path>` and `CAT <path>`.
Replies: `OK <names…>` (LS), `OK <content>` (CAT),
`ERR not_found`, `ERR bad_path` (unknown opcode/malformed),
`ERR too_long` (reserved: content would not fit one reply).
Malformed input of any kind gets an `ERR` reply — the service never
crashes on input (no panics possible under the constrained rules; all
parsing is bounded byte comparison).

## 4. Path syntax

Absolute paths only (`/`, `/etc`, `/apps/hello.manifest`), max 59
bytes (64 − `CAT ` − 1), flat two-level namespace, no `.`/`..`, no
trailing slash. Anything else: `ERR not_found` / `ERR bad_path`.

## 5. File table (AXIOM-FS-002)

| path | content (bounded, one reply) |
|---|---|
| `/etc/version` | AxiomRT version/stage line |
| `/etc/limitations` | evaluation-stage limitation line |
| `/apps/hello.manifest` | hello manifest summary |
| `/apps/counter.manifest` | counter manifest summary |
| `/apps/fault_demo.manifest` | fault_demo manifest summary |
| `/docs/about` | one-line project description |

Directories: `/` → `etc apps docs`; `/etc`, `/apps`, `/docs` list
their files. All entries are static `.user.rodata` constants.

## 6. Shell commands (AXIOM-FS-005)

`ls` (= `ls /`), `ls <path>`, `cat <path>`. The shell rewrites the
command into the protocol request (`LS`/`CAT`), forwards it over its
fs endpoint capability, and prints the reply verbatim (the `OK`/`ERR`
prefix stays visible — the protocol is part of the evidence). All
previously existing commands are preserved unchanged.

## 7. Security rules (AXIOM-FS-006)

* New rights bits `fs_read` (1<<5) and `fs_list` (1<<6) ride on the
  fs endpoint capability. **Only shell_service is minted this
  capability.** Apps get none; `fault_demo` in particular cannot even
  reach `fs_service` — its `sys_send` fails closed with `CAP_DENIED`
  before the endpoint is touched (deny-by-default, kernel-enforced).
* Enforcement granularity in this stage is per-endpoint (the kernel
  checks transport rights and must not parse the fs protocol);
  per-operation splitting of `fs_read` vs `fs_list` becomes
  enforceable by `fs_service` itself once IPC carries sender identity
  — stated openly, not hidden.
* Replies are read-only region statics; requests land in a bounded
  stack buffer; both pass the existing validated SUM-gated copy paths.

## 8. Limitations

Read-only; six fixed files; one 64-byte reply per request (contents
sized to fit); no streaming, no metadata, no timestamps; per-endpoint
capability granularity (above); emulator-only, evaluation stage, no
certification claim.

## 9. Future storage transition

`fs_service` keeps the protocol and becomes a client of
`storage_service` (block reads over IPC), adding multi-chunk replies
(`ERR too_long` → continuation) and a real directory structure. The
kernel remains untouched by that transition — that is the point of §1.
