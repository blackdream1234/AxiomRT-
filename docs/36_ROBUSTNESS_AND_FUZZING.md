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
| On-target syscall inventory | dispatcher numbers 1–4, 7–20; other numbers must take the invalid-syscall path |
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
* **Bound:** each register is one `u64`; the dispatcher currently consumes
  syscall numbers 1–4 and 7–20. Numbers 5, 6, and all other values are not
  current on-target operations.
* **Validation/rejection:** dispatch only explicitly recognized numbers.
  Operation-specific indexes, rights, widths, offsets, and states must be
  checked before mutation. Unknown numbers must return the documented
  invalid-syscall outcome against the caller.
* **Containment/survival:** arbitrary scalar arguments must not panic or hang
  the kernel. A documented illegal-syscall user fault may be contained;
  otherwise the result is `SAFE_REJECT`.
* **Coverage:** current host tests cover many operation validators and QEMU
  covers valid trap paths. AXIOM-ROBUST-005 inventories every implemented
  syscall and tests zero, maxima, `u64::MAX`, bad indexes, and invalid
  lifecycle states; AXIOM-ROBUST-013 covers true U-mode adversarial calls.

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
  one-sender bound; QEMU covers cross-AS delivery. AXIOM-ROBUST-003 adds
  boundary-length mutation, repeated no-peer operations, stale state,
  revoked/wrong capabilities, and hostile pointers.

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
* **Coverage:** current host and QEMU capability tests cover primary
  deny-by-default behavior. AXIOM-ROBUST-004 adds all-bits/unknown-bits,
  amplification, restart reuse, wrong endpoint/device, and app attempts to
  reach network/device objects.

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
  liveness checks.

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
  full numeric/grammar/max-length corpus and liveness after rejection.

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
  corpus entries.

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
| Filesystem/storage | AXIOM-ROBUST-006 | host protocol fuzz + QEMU liveness |
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
