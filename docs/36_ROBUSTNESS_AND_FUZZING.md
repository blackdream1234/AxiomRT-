# 36 — Robustness and Fuzzing Model

Document ID: created by AXIOM-ROBUST-001 (Phase v1.8).
Requirement reference: docs/04, docs/06, docs/14, docs/17, docs/25,
docs/28–35.

## 1. Objective and scope

v1.8 systematically attacks existing AxiomRT boundaries. It does not add a
new product feature. A generated case passes only when malformed,
adversarial, exhausted, or repeated input is:

* rejected before unsafe state change;
* contained to the offending U-mode component; or
* handled by an explicit bounded-exhaustion path.

Whenever the fault model requires user-scope containment, the kernel and
unrelated services must remain alive. Robustness evidence is evidence for the
tested model and QEMU paths only. It is not formal proof, production
qualification, real-hardware validation, or certification evidence.

The architecture law remains unchanged. The kernel supplies mechanisms:
traps, scheduling, address spaces, bounded IPC, capabilities, task lifecycle,
timers, MMU, device mediation, and fault containment. Filesystem, storage,
loader, driver, network, shell, and fuzz policy remain in U-mode or host
tools.

## 2. Trust boundaries and attacker model

The following origins are untrusted at their receiving boundary:

* every U-mode register supplied at an `ecall`;
* every user pointer, length, index, identifier, and IPC payload;
* every request received by a U-mode service, even when the current sender is
  another repository-supplied service;
* shell/operator commands and repeated lifecycle requests;
* serial logs, evidence files, and HTTP input consumed by host tools;
* modeled device events and future device-provided values.

Boot-frozen service definitions, kernel-generated task state, capability
objects after successful lookup, and internal table relationships are trusted
kernel state. If untrusted input can corrupt or bypass those relationships,
that is a kernel bug, not an acceptable malformed-input outcome.

The current block and network devices are skeleton/synthetic paths. Testing
them does not establish robustness against real DMA, real interrupts,
hostile hardware, virtio devices, or real network traffic.

## 3. Failure classes

| Class | Required meaning | Acceptable robustness result? |
|---|---|---|
| `SAFE_REJECT` | Input is rejected with a documented bounded error/event before unauthorized memory or state is touched. Other participants are unchanged. | Yes |
| `CONTAINED_USER_FAULT` | The offending U-mode task becomes Faulted/Killed under the documented policy; the kernel and unrelated tasks continue. | Yes, only where the fault contract permits it |
| `BOUNDED_RESOURCE_EXHAUSTION` | A fixed capacity is full or busy and produces an explicit error/drop/block result without overwrite, unbounded allocation, corruption, or starvation outside the documented policy. | Yes |
| `KERNEL_INVARIANT_FAILURE` | Trusted kernel state is inconsistent, an internal bound is violated, the kernel panics/hangs, or user input reaches a path documented as unreachable. | Never |

A fuzz run, host test, or QEMU test that observes
`KERNEL_INVARIANT_FAILURE` fails immediately. It must record the seed and
minimal reproducer. A kernel panic is never reclassified as safe rejection or
user-fault containment.

## 4. Current global bounds

| Resource/interface | Current bound |
|---|---|
| Syscall register | 64-bit syscall number and 64-bit arguments |
| On-target syscall inventory | implemented numbers 1–4 and 7–20; recognized-but-not-implemented stubs 5 (`sys_reply`) and 6 (`sys_cap_query`) return `ERR_NOT_IMPLEMENTED` (-9); every other number takes the invalid-syscall path (`ERR_INVALID_SYSCALL`, -1) |
| IPC message | 128 bytes globally |
| IPC queueing | one rendezvous state per endpoint; no second sender queue |
| Endpoint table | 12 endpoints, IDs 0–11 |
| Capability table | 9 slots per task, indexes 0–8 |
| Tasks / address spaces | 16 TCB slots and 16 user-AS arenas; `MAX_USER_AS` derives from `MAX_TASKS` |
| Service definitions / stacks | 15 service definitions plus init; 16 private 4 KiB stacks |
| User IPC data window | one 4 KiB stack window, `0x20_0000..0x20_1000` |
| Kernel event ring | 32 entries, overwrite-oldest semantics |
| Devices / IRQ routes | 2 device identities and 2 corresponding IRQ routes |
| Block skeleton | 0x200-byte MMIO region, one 4096-byte modeled DMA page |
| Filesystem | bounded IPC; documented path maximum 59 bytes |
| Storage | 64-byte request/reply contract; 8 read-only blocks of 48 bytes |
| Restricted image | 128-byte record transport, 131072-byte maximum described image, exactly one stack page |
| Network | 128-byte service message, 64-byte driver buffers/replies, fixed 64-byte synthetic test packet |
| Console/info copies | console write at most 256 bytes; info staging at most 768 bytes |
| Watchdog | four missed tick windows before containment |

These values are inputs to AXIOM-ROBUST-011. Related capacities must derive
from one source of truth where feasible. A trusted static definition naming
an out-of-range task, endpoint, device, stack, or IRQ route is
`KERNEL_INVARIANT_FAILURE`, not resource exhaustion.

## 5. Surface threat model and coverage plan

“Current coverage” names evidence already present before v1.8. “Planned”
names the AXIOM-ROBUST task that must add deterministic adversarial coverage.
A plan is not a claim that the case already passes.

### 5.1 Syscall numbers and scalar arguments

* **Source/trust boundary:** arbitrary U-mode values in `a7` and argument
  registers cross into the trap/dispatcher boundary.
* **Bound:** each register is one `u64`. Every number is deterministically
  in exactly one of three classes (verified against the live dispatch path
  by AXIOM-ROBUST-005):
  - **implemented:** 1–4 and 7–20, consumed by the active dispatcher;
  - **recognized-but-not-implemented:** 5 (`sys_reply`) and 6
    (`sys_cap_query`). The active dispatcher passes them to the legacy
    dispatch layer, which acknowledges them as ABI-recognized stubs and
    returns `ERR_NOT_IMPLEMENTED` (-9). They are **not** invalid numbers
    and must never alias another operation or mutate any state;
  - **invalid:** every other value, rejected with `ERR_INVALID_SYSCALL`
    (-1) and no state change.
* **Validation/rejection:** dispatch only explicitly recognized numbers.
  Operation-specific indexes, rights, widths, offsets, and states must be
  checked before mutation. Unknown numbers must return the documented
  invalid-syscall outcome against the caller.
* **Containment/survival:** arbitrary scalar arguments must not panic or hang
  the kernel. A documented illegal-syscall user fault may be contained;
  otherwise the result is `SAFE_REJECT`.
* **Coverage:** current host tests cover many operation validators and QEMU
  covers valid trap paths. AXIOM-ROBUST-005 inventories every syscall
  number and tests zero, maxima, `u64::MAX`, bad indexes, and invalid
  lifecycle states (section 5.1.1); AXIOM-ROBUST-013 covers true U-mode
  adversarial calls.

#### 5.1.1 Implemented host coverage (AXIOM-ROBUST-005)

The `axiom-fuzz` `syscall` target reuses the shared generator, engine,
result accounting, deterministic artifact, and exact-byte replay path.
Each case holds at most 16 operations, executes twice from fresh state,
and must produce identical counters and final state (SYS-INV-015).
Host-reachable panics are caught and reported as SYS-INV-016 failures.

`SYSCALL_INVENTORY` in `tools/axiom-fuzz/src/targets/syscall.rs` is the
checked three-class inventory of all 20 recognized ABI numbers; its tests
fail if a number changes class, if 1–20 stop being fully recognized, or if
any probed number outside the inventory is not invalid. Adding a runtime
syscall without updating the inventory therefore fails the test suite.
For stub numbers 5/6 the target verifies deterministic
`ERR_NOT_IMPLEMENTED`, no capability/endpoint/task/device mutation, no
partial operation, and no aliasing onto an implemented path; both remain
`SAFE_REJECT`-class outcomes, never invariant failures.

MMIO and DMA bounds decisions are routed through the real host validator
`kernel::device::access_in_bounds` — the exact function the RISC-V
dispatcher calls — covering supported widths {1, 2, 4}, width alignment,
checked `offset + width`, zero-size regions (net0), and `u64::MAX`
overflow probes against the 0x200-byte MMIO window and 4096-byte modeled
DMA page. `kernel::ipc::MSG_MAX_BYTES` supplies the 128-byte IPC bound.
The remaining semantics (fixed-order capability checks over the 9-slot
tagged runtime array, the `0x200000..0x201000` user stack window, task and
service lifecycle rules, endpoint rendezvous, IRQ routes, and the
record-only fault acknowledgement) are a documented host model of the
private riscv64 dispatcher, exercised with a 56-scenario named boundary
bank (invalid/stub/implemented numbers, capability slots 0/8/9/`u64::MAX`
plus empty/revoked/wrong-type/wrong-object/wrong-rights, endpoints
0/11/12/`u32::MAX`, tasks 0/15/16/`u64::MAX`, services first/last/
one-beyond/`u64::MAX`, MMIO and DMA first/last-valid/boundary/misaligned/
bad-width/overflow/direction-mismatch, pointer null/valid/kernel/
cross-range/overflow, lifecycle start/kill/restart including repeats and
self-restart, and the four resolved fault-ack cases). SYS-INV-001 through
SYS-INV-018 enforce total classification, stub and invalid-number
no-mutation, bounds before indexing, authority before mutation, checked
arithmetic, no rejected-state change, determinism, and panic-free host
execution.

Resolved contract decisions recorded by AXIOM-ROBUST-005:

* Syscalls 5/6 are ABI-recognized stubs returning -9. The previous
  revision of this document wrongly listed them on the invalid-number
  path; the runtime ABI was not changed — only this document and the
  robustness inventory were corrected.
* `sys_fault_ack` (7): the on-target contract is the docs/19 §4
  record-only acknowledgement (`a1` = decision; 2 = Kill, 1 = Restart,
  other values are recorded as Escalate; deterministic 0). The
  docs/04 event-ID/pending-fault contract (`ERR_NO_PENDING_FAULT`)
  belongs to the host fault-event model (`kernel::fault::wire`) — the
  on-target one-byte fault notification carries no event ID by design.
  Inspection found one real runtime defect: every boot policy mints the
  supervisor's fault-channel capability with the Control right
  (docs/04's required authority), but the live syscall accepted every
  caller, letting capability-less tasks forge
  `RECOVERY_APPLIED` evidence and spam the bounded event ring. The
  fuzz target encodes the intended authority gate (SYS-INV-017/018);
  the minimal runtime fix lands separately as AXIOM-ROBUST-005B.

This remains host-model evidence: the target executes no RISC-V trap
frame, SATP/address-space switch, SUM-mediated copy, or the private
runtime `valid_user_buf`/`in_readable_window`/`in_stack_window`
validators, and it emits no `CONTAINED_USER_FAULT` because no real U-mode
trap occurs. Null/kernel/unmapped/cross-page pointers against the real
MMU, true U-mode adversarial calls, and the runtime 9-slot capacity proof
remain assigned to AXIOM-ROBUST-013 and AXIOM-ROBUST-011.

### 5.2 User pointers and lengths

* **Source/trust boundary:** U-mode supplies source/destination addresses and
  lengths to IPC, console, info, and device-information copies.
* **Bound:** IPC is 128 bytes, console write is 256 bytes, info staging is
  768 bytes, and user stack buffers must remain wholly within one mapped
  4 KiB window. Read-only sectioned U-mode data is accepted only by the
  documented readable-region rule.
* **Validation/rejection:** use checked range arithmetic; reject null,
  kernel, unmapped, overflowing, end-crossing, or over-limit ranges before
  setting SUM or copying. Alignment is checked where the operation requires
  it.
* **Containment/survival:** validation failure is `SAFE_REJECT`; an actual
  illegal U-mode dereference may be `CONTAINED_USER_FAULT`. A supervisor
  copy fault or partial out-of-bounds copy is
  `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** current memory/page-table host tests and memory-isolation
  QEMU tests cover representative boundaries. AXIOM-ROBUST-005 and -013 add
  the full pointer/length matrix, including cross-page and
  cross-address-space cases.

### 5.3 Bounded IPC payloads and rendezvous state

* **Source/trust boundary:** any endpoint-capable U-mode task supplies
  payload bytes, buffer lengths, and operation ordering.
* **Bound:** 128 bytes; one endpoint state is Idle, SenderWaiting, or
  ReceiverWaiting; no heap and no second sender queue.
* **Validation/rejection:** capability/type/right checks precede endpoint
  access; oversized length is rejected before copy; buffers are range
  checked; a busy endpoint and killed/cancelled peer take explicit bounded
  paths.
* **Containment/survival:** malformed traffic must be `SAFE_REJECT` or
  `BOUNDED_RESOURCE_EXHAUSTION`. Cancellation must clear stale waits and
  never expose another address space. No user-controlled IPC input may
  panic the kernel.
* **Coverage:** current IPC/capability host suites cover copy semantics,
  blocking, cancellation, rights, deterministic histories, and the
  one-sender bound; QEMU covers valid cross-AS delivery. AXIOM-ROBUST-003
  implements deterministic host fuzzing for bounded payload, rendezvous,
  cancellation, capability, and task-state behavior. Hostile user pointers
  remain explicitly deferred to AXIOM-ROBUST-013.

#### 5.3.1 Implemented host coverage (AXIOM-ROBUST-003)

The `axiom-fuzz` `ipc` target calls the real public host model rather than
a duplicate protocol implementation:

* `kernel::ipc::MSG_MAX_BYTES` is imported directly as the 128-byte source
  of truth. The target exercises 0, 1, maximum-minus-one, maximum,
  maximum-plus-one, and a bounded 512-byte requested length.
* `Message::new`, `send_checked`, `recv_checked`, and `cancel` supply
  the actual copy, capability, endpoint-binding, rendezvous, and cancellation
  behavior. Source bytes are mutated after message construction and delivered
  bytes must remain exact.
* `Endpoint` supplies the actual Idle, SenderWaiting, and ReceiverWaiting
  state machine. A second sender or receiver must return Busy/AlreadyWaiting;
  no operation queue is modeled or added.
* Three per-task `CapTable` instances exercise valid Send/Receive, missing,
  revoked, wrong-endpoint, wrong-object-type, and insufficient-right cases.
  The fuzz adapter uses slot 0 only. The generic host table has 32 slots while
  the current runtime table has 9; AXIOM-ROBUST-003 makes no capacity claim
  across that known representation difference.
* Real `Thread` transition rules provide Ready, Running, Blocked, Faulted,
  and Killed checks. The adapter applies explicit send/receive outcomes to
  these threads and verifies cancellation removes the single blocked state.
  The host Endpoint API itself does not own scheduler state.

Each case contains at most 16 operations. A fixed 20-scenario boundary bank
guarantees sender-first, receiver-first, repeated send/receive, both
cancellation directions, cancellation after delivery, capability denials,
terminal tasks, exact maximum copies, and oversized rejection. Remaining
input bytes decode deterministically into Send, Receive, Cancel, RevokeCap,
RestoreCap, wrong endpoint/type/rights, fault, kill, and make-ready
operations. The target replays every decoded sequence from a fresh state and
requires the same counters and final snapshot (IPC-INV-012).

Normal validation failures are `SAFE_REJECT`. Busy/AlreadyWaiting outcomes
are `BOUNDED_RESOURCE_EXHAUSTION` because they expose the documented
one-party endpoint capacity. Any IPC-INV-001 through IPC-INV-013 violation is
`KERNEL_INVARIANT_FAILURE` and therefore uses the common deterministic
artifact and exact-byte replay path. No `CONTAINED_USER_FAULT` is assigned
to ordinary validation errors.

This is host-model evidence. The target does not execute a RISC-V trap frame,
SATP/address-space switch, SUM-mediated copy, or the private runtime
`valid_user_buf`/`in_readable_window` validators. The existing IPC QEMU
test remains the evidence for one valid cross-address-space rendezvous; it
does not prove malformed-pointer safety. Null, kernel, unmapped, overflowing,
and cross-page pointer cases remain deferred to AXIOM-ROBUST-013.

**Scope limit made explicit (AXIOM-FOUND-001).** The `ipc` target exercises
`kernel::ipc`, which is a *separate implementation* from the riscv64
dispatcher. It therefore could not and did not detect the dispatcher's
payload-ownership defect, in which a single shared staging buffer let one
send destroy another endpoint's parked message — and let a send that was
subsequently rejected as busy destroy a parked message on its own endpoint.
Host-model campaigns on this target are never evidence about `dispatch.rs`.
The live evidence for dispatcher payload ownership is
`tests/ipc_payload_ownership_qemu_test.sh`, which drives real `ecall`s
through the trap path and checks exact received lengths and bytes, plus
zero-length, maximum-length, oversized rejection, short-receive retry, and
endpoint reuse after a kill.

### 5.4 Capability slots, objects, and rights

* **Source/trust boundary:** U-mode supplies only a slot index; it must never
  fabricate a kernel capability. Boot/service definitions mint the actual
  object ID and rights.
* **Bound:** 9 slots per task. Current endpoint IDs are 0–11 and device IDs
  are 0–1.
* **Validation/rejection:** fixed order: slot range/presence, object type,
  required rights, then object-specific bounds. Unknown rights bits cannot
  satisfy a missing known right. Derivation/reduction may preserve or remove
  authority only.
* **Containment/survival:** invalid, empty, revoked, wrong-type,
  wrong-object, insufficient-right, or cross-task attempts are
  `SAFE_REJECT` with capability-denial evidence and no target state
  change.
* **Coverage:** AXIOM-ROBUST-004 implements deterministic host capability
  fuzzing for the adversarial matrix below. Existing QEMU denial evidence
  remains separate; no runtime code changed and no new QEMU claim is made.

#### 5.4.1 Implemented host coverage (AXIOM-ROBUST-004)

The `axiom-fuzz` `capability` target reuses the shared generator, engine,
result accounting, deterministic artifact, and exact-byte replay path. Each
case has at most 16 operations, executes twice from fresh state, and must
produce identical counters and final state (CAP-INV-014). Host-reachable
panics are caught and reported as CAP-INV-015 failures.

The target routes authority decisions through real public kernel APIs:

* `CapTable::insert`, `lookup`, `query`, and `revoke`; `Capability`,
  `ObjectRef`, `ObjectType`, `Rights`, and `derive_diminished` for generic
  endpoint/task authority;
* `send_checked`, `Message`, and `Endpoint` for wrong/revoked endpoint
  attempts and unchanged rendezvous state;
* `DeviceTable::check`, `DeviceCapability`, `DeviceId`, and `DeviceRights`
  for block0/net0 presence, identity, and rights checks.

A named 35-scenario bank guarantees valid, empty, invalid, repeated,
clear/revoke/double-revoke, runtime-slot boundary, host-slot boundary,
wrong type/control type/object/endpoint/device, nonexistent device,
missing/zero/all-known/all-bits/unknown rights, equal/subset/empty/superset
derivation, cross-task slot reuse, kill/fault, restart, capability-less
application, valid-device, and full-host-table cases. Remaining input bytes
decode deterministically into Lookup, LookupAfterRevoke, Derive,
ReduceRights, AttemptAmplification, ReplaceSlot, ClearSlot, CrossTaskUse,
wrong binding, rights, revoke/restore/restart, lifecycle, device-use, and
fill-table operations.

CAP-INV-001 through CAP-INV-015 enforce absence, revocation, type/object
binding, required rights, unknown-bit rejection, monotonic derivation,
per-task isolation, no rejected-target mutation, immediate revocation,
boot-bounded restart, deterministic state, and panic-free host execution.
Ordinary denials are `SAFE_REJECT`; only filling all 32 generic host slots
is `BOUNDED_RESOURCE_EXHAUSTION`; this target emits no
`CONTAINED_USER_FAULT` for validation errors.

This remains host-model evidence, with deliberate representation limits:

* generic `CapTable` capacity is 32, but the private runtime `Cap` array has
  9 slots (0-8); host slot 9 is therefore only an observed representation
  difference, not runtime one-beyond proof;
* host device capabilities are a separate typed API, while runtime endpoint,
  console, control, info, and device caps share one tagged 9-slot array;
* generic host `Rights` exposes the eight bits 0-7. Runtime endpoint caps
  also carry boot-only filesystem/storage/network policy bits 8-13. Unknown
  raw patterns cannot be constructed through the private host types, so the
  fuzz input adapter rejects them before minting and verifies they cannot
  substitute for a missing known right;
* the restart adapter rebuilds representative service/manager boot caps and
  an empty application profile. The live runtime preserves its statically
  boot-minted cap array when re-arming a task and exposes no U-mode
  mint/derive/replace syscall; this target does not execute that RISC-V path;
* inspection confirms 12 runtime endpoints (IDs 0-11), 2 devices (IDs 0-1),
  and no boot-granted network/device authority for ordinary applications or
  `fault_demo`, but the representative host profiles are not a proof of the
  complete boot service table.

The existing `tests/capability_qemu_test.sh` remains evidence for one live
capability-less IPC denial and unchanged endpoint state. This task did not
rerun or extend QEMU because no runtime defect or runtime change was found.
Live syscall-slot mutation, exact 9-slot capacity, boot-cap installation,
restart-table behavior, and integrated application/device/network authority
remain assigned to AXIOM-ROBUST-005, -011, -012, and -014.

### 5.5 Endpoint identifiers and endpoint state

* **Source/trust boundary:** normal IPC syscalls do not accept raw endpoint
  IDs; they resolve a user slot to a kernel-minted endpoint capability.
  Endpoint IDs inside boot definitions are trusted static input.
* **Bound:** 12 endpoint objects and one rendezvous state per object.
* **Validation/rejection:** user slot validation occurs before lookup.
  Static endpoint IDs and device IRQ endpoints must be proven below
  `NUM_ENDPOINTS`; a malformed trusted definition is not user input.
* **Containment/survival:** wrong user capabilities are `SAFE_REJECT`;
  full/busy rendezvous is `BOUNDED_RESOURCE_EXHAUSTION`. An out-of-range
  trusted endpoint reaching array indexing is `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** AXIOM-ROBUST-003/004 mutate accessible slots;
  AXIOM-ROBUST-011 adds static relationships and full/one-beyond capacity
  tests.

### 5.6 Task IDs and lifecycle transitions

* **Source/trust boundary:** control-capable U-mode services supply service
  table indexes or task slots to start, kill, and restart mechanisms.
* **Bound:** 15 service definitions address slots 1–15; init occupies slot
  0; total task/address-space capacity is 16.
* **Validation/rejection:** require task-control authority; validate table or
  slot bounds, non-empty state, restartability, and no self-restart before
  mutation. Kill/fault/restart clears pending IPC and owned endpoint waits.
* **Containment/survival:** invalid transitions are `SAFE_REJECT`; a full
  table is `BOUNDED_RESOURCE_EXHAUSTION`. Cross-task corruption, stale
  execution after kill, or an address-space capacity mismatch is
  `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** lifecycle and scheduler host tests plus app/driver/network
  QEMU tests cover representative transitions. AXIOM-ROBUST-005, -011, and
  -012 add invalid and repeated transition sequences.

### 5.7 Filesystem protocol

* **Source/trust boundary:** shell and loader requests enter the U-mode
  `fs_service`; the kernel treats bytes as opaque IPC data.
* **Bound:** global transport maximum 128 bytes; documented absolute path
  maximum 59 bytes; one bounded reply.
* **Validation/rejection:** recognize exact `LS <path>` and `CAT <path>`
  forms; reject empty, truncated, unknown, relative, malformed, overlong,
  or nonexistent paths with deterministic `ERR` replies.
* **Containment/survival:** malformed bytes are `SAFE_REJECT`. A parser
  fault must be `CONTAINED_USER_FAULT` to `fs_service`; the kernel,
  shell, storage, apps, and other drivers must survive.
* **Coverage:** current filesystem and restricted-loader QEMU tests cover
  valid and selected invalid paths. AXIOM-ROBUST-006 adds deterministic
  empty/whitespace/NUL/separator/length/boundary mutation and post-reject
  liveness checks (section 5.7.1).

#### 5.7.1 Implemented host coverage (AXIOM-ROBUST-006)

The `axiom-fuzz` `fs` target models the docs/28 §3/§4 protocol in the
same prefix order as the live `fs_body`, including the docs/33
storage-backed `/bin` bridge: `CAT /bin/<app>.app` performs a nested
storage read through this crate's storage model and strips the
`OK data=` frame, while `CAT /storage/version` forwards the storage
reply verbatim — the asymmetry the runtime actually implements.

Correctness is checked differentially. An independent path oracle
re-derives the expected reply for every request from the documented
tables, and the model's answer must match byte for byte. FS-INV-001
bounds the transport (64-byte request window; replies bounded by the
imported `kernel::ipc::MSG_MAX_BYTES`, since the `/bin` listing is 103
bytes); FS-INV-002/003 require exact listing and file content with no
aliasing between paths; FS-INV-004 requires that a storage failure
answers `ERR not_found` and never partial or invented content;
FS-INV-005 requires deterministic replay; FS-INV-006 forbids
host-reachable panics. Any violation is `KERNEL_INVARIANT_FAILURE`
with the usual artifact and exact-byte replay.

A named 32-scenario bank pins every documented path plus unknown,
relative, trailing-slash, directory-as-file, `..`, case-mutated,
prefix-extended, NUL-bearing, empty, whitespace-only, 59-byte
(documented maximum), 60-byte (last fitting) and 65-byte
(transport-exceeding) requests, and a storage-down/-up cycle proving
the failure path and its recovery. Remaining input bytes decode into
further `LS`/`CAT` requests over the known path vocabulary with
deterministic bit-flip, truncation, NUL-insertion and append mutations,
plus storage-availability toggles.

This is host-model evidence for the protocol only. The live U-mode
service, its SUM-gated copies, and the real IPC transport remain
covered by the filesystem, storage, and restricted-loader QEMU tests.

### 5.8 Storage protocol

* **Source/trust boundary:** shell and `fs_service` requests enter U-mode
  `storage_service`; block-number text is untrusted.
* **Bound:** 64-byte request/reply contract; 8 blocks, each 48 bytes;
  read-only and single-block response.
* **Validation/rejection:** accept exact `INFO`, `READ block=<n>`, and
  `READ_RANGE start=<n> count=<m>`; checked decimal parsing rejects
  overflow, negative-looking input, junk, bad blocks, and multi-block
  requests with bounded errors.
* **Containment/survival:** malformed requests are `SAFE_REJECT`; service
  failure is contained in U-mode. The kernel and unrelated services must
  survive.
* **Coverage:** current host parsing helpers and storage QEMU tests cover
  normal geometry and selected malformed cases. AXIOM-ROBUST-006 adds the
  full numeric/grammar/max-length corpus and liveness after rejection
  (section 5.8.1), and found one real runtime defect (section 5.8.2).

#### 5.8.1 Implemented host coverage (AXIOM-ROBUST-006)

The `axiom-fuzz` `storage` target models the docs/29 §4 protocol in the
same order as the live `storage_body`, with the block image mirrored
byte for byte from the runtime statics. An independent grammar oracle
re-derives the expected answer for every request; the model must match
it exactly. STOR-INV-001 bounds the 64-byte request and reply;
STOR-INV-002 requires exact block content for every in-range read;
STOR-INV-003 requires the documented error for every malformed or
out-of-range request; STOR-INV-004 requires checked decimal arithmetic
(section 5.8.2); STOR-INV-005 requires deterministic replay;
STOR-INV-006 forbids host-reachable panics; STOR-INV-007 forbids any
answer that is neither the documented content nor the documented error.

A named 30-scenario bank pins `INFO` (exact and with trailing junk),
blocks 0/7/8, leading zeros, `u64::MAX`, 2^64, a 26-digit number,
negative-looking input, missing digits, embedded and trailing junk,
lowercase opcodes, every `READ_RANGE` boundary (count 0/1/2, overflowing
start and count, missing and corrupted ` count=` separator), empty,
whitespace, NUL-bearing and binary requests, a request of exactly 64
bytes, and one of 65 bytes that the bounded IPC path refuses before the
service parses anything.

#### 5.8.2 Decimal overflow defect (AXIOM-ROBUST-006B)

`parse_dec_stop` accumulated decimal digits with `wrapping_mul`/
`wrapping_add` and accepted the wrapped result. A number larger than
`u64::MAX` therefore wrapped into a valid-looking value: `READ
block=18446744073709551616` (2^64) wrapped to 0 and the service
answered with **block 0's real content** instead of an error — a
wrong-answer defect, not a rejection defect, reachable from the shell
as `storage read 18446744073709551616`.

The same helper backs `ld_num_after`, which parses the AXAPP1 record
fields (`entry`, `text`, `rodata`, `stack`, image size) that
`ld_validate` range-checks (docs/32 §6 rule 5). A wrapped field value
could therefore present an absurd size as an accepted small one; the
record checksum is a plain additive sum and cannot prevent it. The
current `/bin` records are static, so this second path is not
user-reachable today, but the validator is specified to treat records
as untrusted input.

AXIOM-ROBUST-006B fixes this at the root by bounding the **digit
count**: at most 19 digits are accepted. Nineteen nines
(9 999 999 999 999 999 999) is below `u64::MAX`, so a number within the
budget can never overflow, and a longer one returns the existing "not a
number" sentinel — both callers then take their existing
`ERR malformed` / `bad_image` path unchanged. All in-range behaviour is
identical.

The bound is deliberately a digit count rather than a value comparison.
The first attempt compared against `(u64::MAX - 9) / 10`; LLVM
materialised that 64-bit constant as a pc-relative load from kernel
`.rodata`, which sectioned U-mode code must never reference (section 8).
`storage_service` page-faulted at `stval≈0xf088` on the first live boot
— the fourth instance of this failure mode in the project, and the
reason the QEMU test is run for every U-mode change. The consequence of
the digit rule is that a number written with more than 19 digits is
rejected even when leading zeros would make its value small; every real
field is far below the limit (blocks 0–7, sizes ≤ 131072), and the host
model encodes exactly the same rule.

`tests/storage_service_qemu_test.sh` now probes both overflow forms on
the live target and asserts that block 0's content appears exactly once
(from the legitimate `storage read 0`), so a regression that re-aliases
an out-of-range number fails the suite.

#### 5.8.3 Empty-request behaviour (both protocols)

Cross-checking the models against the runtime showed one further
difference, in the models rather than the kernel: both services test
`if r <= 0 { continue }` after receiving, so a **zero-length message is
consumed and produces no reply at all** — not an `ERR malformed` /
`ERR bad_path`. The models now represent this as a distinct `Ignored`
outcome. No in-tree client can send one (every sender writes at least an
opcode prefix), but a client that did would wait for a reply that never
arrives; that is a liveness property of the caller, not a kernel fault,
and it is recorded here rather than silently normalised away.

### 5.9 Restricted application loader

* **Source/trust boundary:** app names and storage-backed `AXAPP1` record
  bytes enter U-mode loader policy; mapping admission crosses into
  kernel mechanisms only after validation.
* **Bound:** record transport is at most 128 bytes; described text+rodata is
  at most 131072 bytes; stack policy is exactly one page; arithmetic is
  checked before address construction.
* **Validation/rejection:** fixed-order magic/version, field grammar,
  checksum, layout/entry/image-size/stack/W^X separation, capability
  vocabulary/policy, known-app, and state checks. Rejection must leave no
  partially installed app or authority.
* **Containment/survival:** invalid records are `SAFE_REJECT`. A loader
  parser fault is a contained U-mode fault; mapping kernel memory, mapping a
  user W+X page, granting unauthorized authority, or leaving partial state is
  `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** current loader/image/page-table host tests and restricted
  loader QEMU test cover representative good and bad records.
  AXIOM-ROBUST-007 mutates every field, integer boundary, name, trailing
  byte, capability request, and load/unload cycle with pinned regression
  corpus entries (section 5.9.1).

#### 5.9.1 Implemented host coverage (AXIOM-ROBUST-007)

The `axiom-fuzz` `loader` target models the docs/32 §6 AXAPP1
validation contract in the order `ld_validate` actually applies it, and
the docs/33 §7 fetch path is routed through this crate's `fs` model so
the `/bin` record vocabulary has one source of truth. Two properties
make it more than a re-implementation:

* mapping admission is decided by the **real** kernel host API
  `kernel::loader::admit_image_mapping`. LD-INV-005 requires that every
  record the loader accepts describes a layout the kernel mapping
  mechanism would itself admit — entry inside text, bounded image, one
  stack page, span entirely below `KERNEL_BASE`, W^X by separation. A
  validator that drifted looser than the mechanism would fail here.
* an independent field-splitting oracle re-derives the verdict for every
  record, so a *wrong verdict* — not only a crash — is an invariant
  failure (LD-INV-002).

The remaining invariants: LD-INV-001 record and transport bounds (a
record over 64 bytes never reaches the validator, because `ld_fetch`
receives into 64); LD-INV-003 **no partial install** — a rejected record
leaves the loader's state, grants and start counters byte-identical;
LD-INV-004 no authority beyond per-app policy, and an unload drops the
grant; LD-INV-006 the checksum cannot be bypassed and does not certify
grammar; LD-INV-007 lifecycle legality; LD-INV-008 deterministic replay;
LD-INV-009 no host-reachable panic; LD-INV-010 an unknown name can never
produce a valid verdict.

A named 40-scenario bank pins the three canonical records, the three
static `/bin` fixtures, magic/version corruption, record lengths 12, 13,
64 and 65, every checksum failure mode (wrong digit, uppercase hex,
non-hex, three digits, missing separator), every layout boundary
(`entry == text`, `entry > text`, `entry = u64::MAX`, `text = 0`,
`text` at and over 65536, `rodata` over 65536, `stack` 0 and 2,
`image != text + rodata`, `image` over 131072), the capability
vocabulary including an excessive request by `fault_demo`, and the
structural cases missing field, extra field, double space and trailing
byte. Every case also runs one of eight lifecycle sequences:
`load → state → unload → load`, duplicate load, unload when absent, run
before load, reload after exit, reload after fault, invalid-then-valid
load of the same app, and mapping-admission probes at the policy edges.

Six pinned corpus entries live in `tools/axiom-fuzz/corpus/loader/`
(layout boundaries, capability policy, mapping admission, record
mutations, lifecycle mix, name vocabulary). The directory holds raw
input bytes only — `Corpus::load` takes every regular file in the
directory, so no README may be placed there. Use it with
`--corpus tools/axiom-fuzz/corpus/loader`.

Resolved contract question: docs/32 §6 previously numbered the checks
with the field grammar before the checksum, but `ld_validate` verifies
the checksum first and documents why ("so a corrupt record never drives
the parser"). The runtime order is intentional and strictly safer — the
only observable difference is that a record which is both malformed and
mis-checksummed answers `ERR bad_checksum` — so docs/32 §6 was corrected
to state the implemented order. No runtime change was made.

This is host-model evidence for loader policy. It does not execute the
live U-mode service, its IPC, or a real address-space construction; the
restricted-loader QEMU test remains the evidence for the live
load/run/reject path, and this task extended it with the duplicate-load,
unload-when-absent, run-before-load and unknown-app probes.

### 5.10 Driver commands and device mechanisms

* **Source/trust boundary:** shell commands enter U-mode
  `driver_manager`; driver requests remain opaque to the kernel. Device
  syscall offsets, widths, and capability slots cross the kernel boundary.
* **Bound:** driver command buffers/replies are 64 bytes; 2 device objects;
  block0 MMIO window is 0x200 bytes; modeled DMA is 4096 bytes; one
  coalesced IRQ state per device route.
* **Validation/rejection:** device capability/type/right checks precede
  device lookup; MMIO/DMA validate supported width, alignment, checked
  offset+width, and granted direction; IRQ raise requires driver-control
  authority.
* **Containment/survival:** invalid mechanisms are `SAFE_REJECT`; busy/drop
  paths are bounded. A deliberate driver U-mode fault is
  `CONTAINED_USER_FAULT`; shell, filesystem, storage, apps, and kernel
  must remain alive.
* **Coverage:** current device host tests and driver-framework QEMU test
  cover core checks and one fault/restart. AXIOM-ROBUST-008 adds invalid
  offsets/directions/devices, repeated IRQ, and repeated fault/restart.
  It makes no real-hardware robustness claim.

### 5.11 Synthetic network protocols

* **Source/trust boundary:** shell bytes enter U-mode `net_service`;
  translated driver bytes enter U-mode `net_driver_service`. The kernel
  parses neither protocol.
* **Bound:** service protocol is at most 128 bytes; driver request/reply
  buffers are 64 bytes; the only packet model is a fixed 64-byte count;
  counters are `u64` with checked host-model overflow.
* **Validation/rejection:** only exact documented `NET_*` and `DRV_*`
  tokens are accepted. Empty, extended, truncated, case-mutated, NUL,
  arbitrary, or oversized input receives a bounded malformed/unsupported/
  too-large outcome.
* **Containment/survival:** malformed traffic is `SAFE_REJECT`; a driver
  fault is `CONTAINED_USER_FAULT`; while down, operations return a bounded
  driver-down result and must not deadlock on an absent receiver.
* **Coverage:** current network host tests and network QEMU test cover exact
  grammar, selected malformed input, containment, and restart.
  AXIOM-ROBUST-009 adds deterministic byte mutation and long repeated
  fault/down/restart/send/stat sequences. This is synthetic testing only,
  not TCP/IP or real-network robustness.

### 5.12 Host event parser and Studio ingestion

* **Source/trust boundary:** serial/evidence text and local HTTP requests
  enter host `axiomctl`/Studio; they do not enter the kernel.
* **Bound:** event parsing currently allocates in proportion to input and has
  no explicit per-line or whole-file event bound. Studio caps a consumed
  HTTP body at 64 KiB, but this is not an event-log size guarantee. This is
  an explicit robustness gap.
* **Validation/rejection:** empty/foreign/unknown lines are skipped rather
  than assigned invented semantics; recognized lines preserve raw data;
  JSON escaping must preserve valid content safely.
* **Containment/survival:** malformed input must be `SAFE_REJECT`/skip and
  must not panic either host tool. Memory growth beyond configured fuzz
  limits is a failed robustness case, not kernel evidence.
* **Coverage:** current axiomctl and Studio unit tests cover selected
  malformed/foreign lines and network classification. AXIOM-ROBUST-010 adds
  bounded huge-line, UTF-8, duplicate/missing field, counter, repetition,
  binary-like, and mixed-stream mutation.

### 5.13 Repeated fault and restart sequences

* **Source/trust boundary:** an authorized operator or compromised
  control-capable service can repeat app, block-driver, and network-driver
  lifecycle requests.
* **Bound:** each fuzz/QEMU campaign has an explicit cycle limit; underlying
  task, endpoint, capability, IRQ, and event-ring state remains statically
  bounded.
* **Validation/rejection:** already-running, already-down, repeated fault,
  repeated restart, and stale wait states must take documented state-machine
  outcomes. Restart restores only boot-minted authority and clears old IPC.
* **Containment/survival:** deliberate U-mode faults are
  `CONTAINED_USER_FAULT`; invalid repeats are `SAFE_REJECT`; a stale
  endpoint, authority amplification, deadlock, or kernel panic is
  `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** existing QEMU tests cover one cycle. AXIOM-ROBUST-012 adds a
  fixed repeated mixed-service sequence and checks shell/kernel/hello/fs/
  storage/driver/network liveness after every cycle.

### 5.14 Resource exhaustion

* **Source/trust boundary:** repeated valid or adversarial operations attempt
  to consume every fixed table, slot, rendezvous, ring, loader state, or
  scheduler entry.
* **Bound:** the authoritative capacities are listed in section 4.
* **Validation/rejection:** fill and one-beyond cases must return explicit
  no-slot/full/busy/drop errors without indexing past storage. Event-ring
  overwrite-oldest behavior must remain deterministic.
* **Containment/survival:** an expected full condition is
  `BOUNDED_RESOURCE_EXHAUSTION`; corruption, panic, silent overwrite of
  unrelated state, or an independently drifting related capacity is
  `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** current unit tests cover selected scheduler, page-table, and
  IPC capacities. AXIOM-ROBUST-011 audits all static capacities, adds
  compile-time relationships where possible, and tests fill/one-beyond and
  repeated restart cases. The v1.7 `MAX_TASKS`/`MAX_USER_AS` regression
  must remain permanently guarded.

### 5.15 Scheduling and CPU stress

* **Source/trust boundary:** U-mode tasks control when they yield, block,
  issue syscalls, or consume CPU; priorities and task definitions are
  trusted boot policy.
* **Bound:** 16 task slots; fixed-priority selection scans the bounded TCB
  array; equal-priority ordering is deterministic; watchdog containment
  follows four missed tick windows.
* **Validation/rejection:** only Ready tasks may run; Blocked, Faulted, and
  Killed tasks must never be selected even if stale bookkeeping exists.
  User actions cannot change priority or create an unbounded ready queue.
* **Containment/survival:** CPU exhaustion is
  `CONTAINED_USER_FAULT` through watchdog policy. Starvation contrary to
  documented priority policy, execution of terminal tasks, deadlock caused
  by stale IPC, or scheduler corruption is `KERNEL_INVARIANT_FAILURE`.
* **Coverage:** current scheduler host tests and preemption/watchdog/full-demo
  QEMU tests cover primary properties. AXIOM-ROBUST-003, -011, -012, and
  -014 add stale-state, full-capacity, fault-storm, and integrated scheduling
  stress.

## 6. Coverage ownership

| Surface | Primary v1.8 task | Required level |
|---|---|---|
| IPC and rendezvous | AXIOM-ROBUST-003 | host deterministic fuzz + targeted QEMU |
| Capabilities | AXIOM-ROBUST-004 | host adversarial matrix + QEMU denial |
| Syscall scalars | AXIOM-ROBUST-005 | host validators + QEMU |
| User pointers | AXIOM-ROBUST-005 / -013 | host boundary tests + dedicated QEMU task |
| Filesystem/storage | AXIOM-ROBUST-006 | host protocol fuzz + QEMU liveness (done: sections 5.7.1, 5.8.1, 5.8.2) |
| Restricted loader | AXIOM-ROBUST-007 | host field mutation + QEMU corpus |
| Driver/device | AXIOM-ROBUST-008 | host bounds + QEMU containment |
| Synthetic network | AXIOM-ROBUST-009 | host protocol fuzz + QEMU recovery |
| Event ingestion | AXIOM-ROBUST-010 | host fuzz for axiomctl and Studio |
| Static exhaustion | AXIOM-ROBUST-011 | host fill/one-beyond + static assertions |
| Fault/restart storms | AXIOM-ROBUST-012 | deterministic QEMU stress |
| Integrated release behavior | AXIOM-ROBUST-014 / -015 | release QEMU + full sweep |

AXIOM-ROBUST-002 supplies the deterministic harness used by host targets.
Every run records target, seed, iteration count, corpus, and maximum length.
Ordinary verification uses a bounded smoke count; AXIOM-ROBUST-016 supplies
an explicit deep mode. Every discovered failure receives a minimized,
checked-in deterministic regression input before the fix is considered
complete.

### 6.1 Deterministic host harness (AXIOM-ROBUST-002)

The reusable host-only harness is the zero-dependency `axiom-fuzz` crate
under `tools/axiom-fuzz`. It uses the fixed SplitMix64 algorithm as a
testing PRNG; this is not cryptographic randomness. A run requires an explicit
seed and never consults OS randomness, wall-clock time, threads, or the
network. For fixed target, seed, iteration count, `max_len`, and corpus
contents, cases are generated in the same order and the printed FNV-1a digest
is identical.

The only AXIOM-ROBUST-002 target is `smoke`. It validates the harness using
empty, boundary, generated-byte, bit/insert, and corpus-derived cases without
calling AxiomRT runtime logic:

```sh
cargo run -p axiom-fuzz --target x86_64-unknown-linux-gnu -- \
  --fuzz-target smoke --seed 20260903 --iterations 1000 --max-len 128
```

Optional `--corpus <directory>` loads only direct regular-file children in
sorted filename order. An empty directory is valid. Files above `max_len`
are rejected, not silently truncated; entry count and total retained corpus
bytes are also capped. Generated inputs never exceed `max_len`. The harness
hard-caps `max_len` at 1 MiB, one invocation at 10,000,000 iterations,
corpus count at 4096 files, and retained corpus data at 16 MiB.

A `KERNEL_INVARIANT_FAILURE` stops generation, exits non-zero, and writes a
timestamp-independent text artifact beneath
`fuzz_failures/<target>/seed-<seed>-iteration-<iteration>.txt`. The artifact
records target, seed, iteration, exact input hex and length, mutation, corpus
origin, and reason. Replay loads the stored bytes directly rather than
regenerating them:

```sh
cargo run -p axiom-fuzz --target x86_64-unknown-linux-gnu -- \
  --replay fuzz_failures/smoke/seed-20260903-iteration-0.txt
```

CI smoke runs use a small fixed iteration count. Larger local campaigns remain
explicit and bounded by their command line. Deep-mode policy, protocol
targets, minimized checked-in regression corpora, and final `verify_all`
integration belong to later AXIOM-ROBUST tasks. Passing this harness is
robustness evidence for the exercised cases, never proof of correctness.

### 6.2 Verification-runner integrity (AXIOM-PLAN-003)

`scripts/verify_all.sh` is itself part of the verification argument: a runner
that cannot report failure turns every downstream claim into an unchecked
assertion. AXIOM-PLAN-003 addresses runner integrity only, in this bounded
scope:

* the final "restore default build" is accounted for in the aggregate result
  instead of being run with its status and output discarded, so a broken
  default build can no longer coexist with `VERIFY ALL: PASS`. Its diagnostics
  stay visible;
* the existing child-status propagation, the QEMU suites, the four
  existing host suites and the Coq step are preserved unchanged;
* `cargo test -p axiom-fuzz` is executed, so the fuzz host tests are covered;
* seven bounded smoke campaigns run through the documented CLI — `smoke`,
  `ipc` and `capability` at seed 20260903, `syscall`, `storage`, `fs` and
  `loader` at seed 20260904, each 200 iterations at `--max-len 128`, with
  `loader` additionally seeded from `tools/axiom-fuzz/corpus/loader`. These
  parameters are this task's operational selection, not a historical claim;
  failure artifacts are written under `target/axiom-plan-003/fuzz-failures/`;
* a missing prerequisite (`cargo`, `qemu-system-riscv64`, `coqc`, `timeout`)
  produces an explicit diagnostic, the terminal line `VERIFY ALL: BLOCKED`
  and exit status 2 **before any suite runs**. A missing tool is never
  reported as a pass and never silently skipped;
* every child command runs under an operational timeout (600 s by default,
  10 s kill-after grace, overridable through a positive-integer
  `SUITE_TIMEOUT_S`; an invalid value is rejected before any suite runs). A
  timeout or forced termination is reported with its status and command and
  can never yield a pass. This bound is operational scheduling hygiene, not a
  timing guarantee — see section 9 and AXIOM-ROBUST-013/EVID timing work.

Exit statuses are 0 for a complete successful sweep, 1 for an executed
verification failure, and 2 for a blocked prerequisite or invalid
configuration. The terminal strings `VERIFY ALL: PASS` and `VERIFY ALL: FAIL`
and the `=== <name> ===` section headers are unchanged, because host tools
consume them.

`tests/verify_all_runner_test.sh` tests the runner itself. It copies the real
script into an isolated temporary root, verifies the copy is byte-identical,
and drives it with command stubs on a closed fixture `PATH`, so no real
`cargo`, QEMU or Coq process runs inside a regression scenario. Stubs record
their invocations, letting the success scenario assert that every suite,
host test, campaign, the Coq step and the final build actually executed — an
omitted step fails the harness. It is run separately and is deliberately not
invoked from `verify_all.sh` in this task. Its results are evidence about the
runner's control flow only and are never kernel evidence.

This task does **not** complete AXIOM-ROBUST-015, -016 or -017. The normative
choice of targets, seeds, iteration counts and corpora for the release sweep,
the documented deep mode, and the v1.8 evidence archive remain open under
those tasks.

#### 6.2.1 Acceptance corrections (AXIOM-PLAN-005)

An acceptance review of the runner-integrity change found that the sweep's
own completeness oracle was weaker than the change claimed, and that two
runner checks drew conclusions the evidence did not support. The corrections
below close those gaps. The seeds, iteration counts, corpora and the
production fuzz-failure path selected earlier are unchanged.

**Invocation accounting now checks semantics, not labels.** The stubs emit one
normalised record per invocation, so every assertion binds its attributes to
the *same* invocation rather than to any line that happens to mention a tool:

* the four package-based host suites, each with its host target;
* the supervisor suite by its exact manifest path *and* host target — it
  carries no `-p` flag, so a package-name check could never have covered it;
* each of `MemoryIsolation.v`, `CapabilityAccess.v` and `SchedulerPriority.v`
  individually, because a search for the compiler name is satisfied by any one
  of the three;
* the final default build together with its `--release` flag;
* every QEMU suite and the seven campaigns with their exact parameters.

Since AXIOM-FOUND-001 the sweep drives **seventeen** QEMU suites: the
original sixteen plus `ipc_payload_ownership_qemu_test`, the live
dispatcher payload-ownership regression. The runner regression fixture
asserts the same seventeen, so a suite dropped from either side fails.

**Negative controls prove the oracle can reject.** Assertions that have only
ever been observed passing prove nothing about their sensitivity. Each
accounting predicate is now paired with a control that deletes exactly the
invocation it is meant to require — the supervisor suite, each Coq unit, and
the required target, manifest and release arguments. A control mutates only a
clearly labelled temporary copy; the production runner is never modified and
the ordinary scenarios continue to run a byte-identical copy. Every control
verifies that its mutation actually took effect and that the mutated copy is
still syntactically valid, because a syntax error or an unrelated failure is
not evidence that an accounting oracle works.

**Setup faults stop before any workload.** Repository-root resolution, the
directory change, the repository markers and the failure-directory creation
are each checked. Any failure prints an explicit diagnostic, `VERIFY ALL:
BLOCKED`, and exits 2 with no suite, host test, campaign, Coq compilation or
build having run — and the preflight scenarios assert that absence across all
of those, not only the QEMU suites.

**Timeout grammar.** `SUITE_TIMEOUT_S` accepts a canonical decimal integer in
`1..86400`; unset means 600 seconds; explicitly empty is invalid. Signs,
non-digits, over-long strings and leading zeros are rejected, and the grammar
and length are validated before any numeric comparison so an oversized value
cannot reach shell arithmetic. Rejecting leading zeros is a chosen canonical
form, not a claim about how any previous version evaluated such a value. The
kill-after grace remains 10 seconds.

**Reported status is separated from inferred cause.** A non-zero outcome
always fails the aggregate gate. Status 124 is reported as the timeout
utility's own indication that the command exceeded the limit, without
asserting an independently established root cause. Status 137 is reported as
a `SIGKILL` with the cause *undetermined*: it can arise from the kill-after
grace, the out-of-memory killer or an external signal, and the number alone
does not distinguish them.

**Harness integrity.** Every fixture-creation step required for safe execution
is checked. A failed `mktemp` aborts before anything derived from that path is
written, so no absolute system path can be touched. Scenario completion is
tracked, so a setup failure cannot silently skip a scenario while the harness
still reports success, and cleanup may remove only a directory this harness
created and validated.

These remain runner-control results. They say nothing about kernel behaviour,
and AXIOM-ROBUST-015, -016 and -017 remain incomplete.

## 7. Reproducibility and evidence rules

* No hidden seed, clock-derived seed, internet corpus, or nondeterministic
  acceptance criterion.
* Time, iterations, message length, and memory use are bounded by command-line
  configuration with safe defaults.
* A failure record contains target, seed, iteration, input bytes, observed
  class, and expected class.
* Crash inputs are never discarded. Valuable minimized cases live in the
  repository corpus.
* `SAFE_REJECT`, `CONTAINED_USER_FAULT`, and
  `BOUNDED_RESOURCE_EXHAUSTION` counts are reported separately.
* Any invariant failure makes the target and overall sweep exit non-zero.
* Host-model results, QEMU runtime results, synthetic device results, and Coq
  model compilation remain separate evidence categories.

## 8. U-mode implementation constraint

Sectioned U-mode code has previously suffered LLVM-generated lookup/jump
tables escaping into kernel-only `.rodata`. New U-mode validation or fuzz
hooks must use small explicit branch helpers and avoid dense/mixed switches
or lookup tables. An unexpected U-mode page fault requires inspection of
generated behavior.

The remedy must never be to map kernel `.rodata` into U-mode. Doing so would
weaken isolation and is forbidden.

## 9. Kernel-survival rule

The kernel must survive every user-generated malformed input. The only
acceptable exception category is not an exception at all:
`KERNEL_INVARIANT_FAILURE` always fails the test and release gate.

Likewise, successful fuzzing does not prove complete memory safety, complete
formal verification, hard-real-time behavior, production readiness,
DO-178C compliance, certification, real-hardware robustness, real-network
robustness, or TCP/IP robustness. AxiomRT remains an emulator-oriented
research/high-assurance prototype.
