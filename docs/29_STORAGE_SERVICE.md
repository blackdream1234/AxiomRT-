# 29 — Storage Service

Document ID: created by AXIOM-STOR-001 (Phase v1.4).
Requirement reference: `AxiomRTos.md` §7, docs/28 §9 (storage
transition), docs/25 §2 (constrained service rules).

## 1. Why storage is user-space

Block management, request parsing, and caching policy are complex and
fault-prone — exactly what the microkernel isolates. A storage bug is
a contained user fault; the kernel contributes only tasks, address
spaces, bounded IPC, capabilities, scheduling, and containment. The
kernel never reads a block and never parses a storage request.

## 2. Embedded filesystem vs block-backed storage

`fs_service` (docs/28) answers by *name* from constants compiled into
its region — no notion of blocks. `storage_service` answers by *block
number* from a block image, the abstraction a real disk driver will
later provide. In this phase the block image itself is still static
(read-only constants); the point is the **protocol and the service
boundary**, so that swapping the backing store for a virtio-blk driver
(docs/30) changes storage_service internals only.

## 3. Responsibilities

Receive bounded IPC requests, parse `INFO` / `READ` / `READ_RANGE`,
validate block numbers against the fixed geometry, return bounded
block data, reject malformed input with `ERR …` (never crash), never
touch kernel memory (hardware-enforced), yield fairly (blocking
rendezvous IPC + preemptive timer).

## 4. Block protocol (AXIOM-STOR-002)

Endpoint 5 (`EP_STOR`), ≤ 64-byte request/reply, one reply per
request. Geometry: **block_size=64, blocks=8, read-only** (a block
fits one reply — no partial reads).

Requests → replies:

* `INFO` → `OK block_size=64 blocks=8 readonly=true`
* `READ block=<n>` → `OK data=<printable-ascii>` | `ERR bad_block`
* `READ_RANGE start=<n> count=<m>` → single-block ranges only in this
  stage: `count=1` behaves like `READ`; `count>1` → `ERR
  too_many_blocks` (a multi-block reply cannot fit one bounded
  message; continuation protocol arrives with the driver phase)
* anything else → `ERR malformed`; unauthorized senders never reach
  the service (`ERR denied` is reserved: the kernel's capability check
  fails such requests closed before delivery)

No dynamic allocation anywhere (constrained rules, docs/25 §2).

## 5. Capability model (AXIOM-STOR-005)

New declarative rights bits on the storage endpoint capability:
`storage_info` (1<<8) and `storage_read` (1<<9). Only
**shell_service** (operator surface) and **fs_service** (for the
`/storage/*` path, §7) hold storage capabilities. Apps hold none;
`fault_demo` holds nothing at all. The capability table grows
(CAPS_PER_TASK 6 → 8) because the shell's table was full at v1.3;
every pre-existing capability is preserved and the storage test
re-checks the `caps` listing.

## 6. Read-only first policy

Writes are refused by construction (no write opcode exists). Future
writable risks, stated now: torn writes vs power loss, block cache
coherence, quota/starvation policy, and a write-authority capability
split — all user-space concerns to be designed in the writable phase,
none of them kernel mechanisms.

## 7. fs_service → storage_service path (AXIOM-STOR-007)

`cat /storage/version` travels
`shell → fs_service → storage_service → fs_service → shell`:
fs_service maps the path to `READ block=1` (its policy), performs the
nested synchronous IPC on its own storage capability, and forwards
the bounded reply. The kernel routes messages and checks capabilities;
it parses neither the path nor the block.

## 8. Static block image (AXIOM-STOR-003)

| block | content |
|---|---|
| 0 | storage header/version (`AXSTOR v1 …`) |
| 1 | `/etc/version` mirror |
| 2 | `/docs/about` mirror |
| 3 | app manifest summary |
| 4–7 | reserved (readable, marked `reserved`) |

All blocks are sectioned `.user.rodata` constants (docs/25 §2).

## 9. Kernel boundary

Kernel additions in this phase: one endpoint id and two rights-bit
constants, plus the capability-table width. No storage logic, no
block access, no request parsing, no geometry knowledge.

## 10. QEMU virtio-blk plan / limitations

The path to a real backing store (QEMU virtio-blk MMIO, device
discovery, the kernel MMIO-grant mechanism it needs, and what stays in
user space) is investigated in docs/30_VIRTIO_BLOCK_INVESTIGATION.md
(AXIOM-STOR-010); the driver itself belongs to the v1.5 driver
framework phase. Limitations: static 8×64-byte image, single-block
replies, read-only, per-endpoint capability granularity (docs/28 §7),
emulator-only, evaluation stage, no certification claim.
