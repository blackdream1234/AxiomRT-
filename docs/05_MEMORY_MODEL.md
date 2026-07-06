# AxiomRT Memory Model

Document ID: AXIOM-DOC-006
Status: Approved for Phase 0

## Enforcement Status

* v0.1: model-level (typed Rust model + host tests).
* **v0.2: MMU-enforced on QEMU for the tested cases** — Sv39 is active;
  a user task that reads kernel memory, writes an unmapped address, or
  executes a non-executable page takes a hardware page fault that is
  contained (docs/12_MMU_SV39.md, tests/memory_isolation_qemu_test.sh).
  Untested access patterns remain a documented gap.

## Core Rules (Normative)

1. No user task can access kernel memory.
2. No user task can access another task's memory without explicit mapping
   authority (and in v0.1 no such shared mapping exists at all).
3. Invalid access creates a page fault.
4. A page fault kills or suspends the offending task (per fault policy) and
   produces a FaultEvent.
5. Shared memory is forbidden in v0.1.

These rules are enforceable and testable; each maps to a verification
property in section 10.

## 1. Address Spaces

* Each task has exactly one AddressSpace; each AddressSpace belongs to
  exactly one task (v0.1).
* An AddressSpace is the total description of what the task can see:
  the set of (virtual range → physical frame, permissions) mappings.
* Anything not explicitly mapped is inaccessible; access attempts trap.
* Address spaces are created at boot from static descriptions in v0.1;
  there is no runtime address-space creation from user space.
* Translation scheme: RISC-V Sv39 (39-bit virtual addresses, 4 KiB pages).

## 2. Kernel Memory

* Kernel code, kernel data, kernel stacks, page tables, capability tables,
  and all kernel object pools live in kernel memory.
* Kernel memory is mapped with KERNEL permission only — never USER.
* There is no code path that maps kernel memory into a user address space;
  the mapping function rejects such requests structurally (not by policy
  check at call sites).
* Kernel memory layout is static after boot: no kernel heap allocation
  after boot completes.

## 3. User Memory

* User memory consists of the task's code, read-only data, data/BSS, and
  stack regions, each mapped with minimal permissions (W^X: no region is
  both WRITE and EXECUTE).
* User regions are mapped USER and never KERNEL.
* A task's virtual layout is defined at boot by its user image descriptor.
* User pointers arriving through syscalls are untrusted data: every syscall
  validates that a user-supplied range lies fully inside the caller's
  mapped user memory with the required permission before any kernel access.

## 4. Physical Frames

* Physical memory is divided into fixed-size frames (4 KiB).
* Every frame has exactly one owner at any time: the kernel, one address
  space, or the free pool.
* Frame ownership uniqueness is an invariant: one frame is never mapped
  into two address spaces (this is the mechanical form of the no-sharing
  rule).
* Frames are allocated at boot in v0.1; freed frames are scrubbed (zeroed)
  before any reuse to prevent data remanence across tasks.

## 5. Page Tables

* Each AddressSpace has one page table tree (Sv39, three levels).
  Hardware activation of Sv39 is specified in docs/12_MMU_SV39.md
  (v0.2, Stage 1): from v0.2 the MMU enforces these rules on target for
  the tested cases, upgrading the isolation claim from model-level.
* Page tables are kernel memory; user tasks can never read or write them.
* The page table is a refinement of the AddressSpace model: every hardware
  entry corresponds to exactly one model mapping. Divergence is a kernel
  invariant violation.
* Only the kernel mapping function writes page table entries; it rejects:
  entries marking kernel memory USER, entries to frames not owned by this
  address space, and entries violating W^X.

## 6. Permissions

Permission flags:

* **READ** — data load allowed.
* **WRITE** — data store allowed.
* **EXECUTE** — instruction fetch allowed.
* **USER** — accessible from user privilege.
* **KERNEL** — accessible from kernel privilege only.
* **DEVICE** — MMIO region; requires an explicit device capability;
  never cacheable, never EXECUTE.

Rules: USER and KERNEL are mutually exclusive on one mapping. WRITE and
EXECUTE are mutually exclusive on user mappings (W^X). DEVICE mappings are
kernel-only in v0.1 (no user-space drivers yet).

## 7. Device Memory

* Device MMIO regions (e.g., UART) are mapped KERNEL + DEVICE in v0.1.
* User tasks get no device mappings in v0.1; device access happens only
  through kernel services reached by syscall/IPC.
* From v0.2+, a user-space driver may receive a DEVICE mapping only through
  an explicit capability with Map right for that specific region.

## 8. Page Fault Behavior

On any invalid access (unmapped address, permission violation, W^X
violation):

1. Hardware traps to the kernel page fault handler.
2. The kernel identifies the faulting thread, address, and access type.
3. The faulting thread is moved to Faulted (never continues at the faulting
   instruction).
4. A PageFault FaultEvent is created: {thread ID, faulting virtual address,
   access type, program counter, severity}.
5. The supervisor is notified; policy decides Kill / Restart / Suspend /
   Quarantine (docs/06_FAULT_MODEL.md).
6. A PAGE_FAULT monitoring event is emitted.

A page fault taken while executing kernel code is a kernel invariant
violation: controlled panic (halt safely), never silent continuation.

## 9. Forbidden Memory Features v0.1

* shared memory between tasks (any form)
* user-controlled mapping or remapping (`mmap`-like syscalls)
* dynamic allocation after boot (kernel heap or user heap growth)
* demand paging, swapping, copy-on-write
* huge pages (4 KiB only, to keep the model small)
* user-space DMA
* executable + writable mappings

## 10. Verification Properties

These are the formal targets derived from this model:

* **MEM-P1 (kernel isolation):** For every reachable system state, no
  mapping in any user address space grants USER access to a kernel frame.
  → proofs/coq/MemoryIsolation.v
* **MEM-P2 (task isolation):** For every pair of distinct tasks A ≠ B, the
  set of frames mapped in A and the set mapped in B are disjoint (v0.1
  no-sharing form).
* **MEM-P3 (fault totality):** Every access outside the mapped, permitted
  set traps; there is no undefined outcome of a memory access.
* **MEM-P4 (frame ownership):** Every frame has exactly one owner; map and
  free preserve this invariant.
* **MEM-P5 (W^X):** No user mapping is simultaneously writable and
  executable.

Test obligations: each property gets at least one negative test (fault
injection attempting to violate it) in the fault-injection suite.

## 11. Model Constants (Phase 4, AXIOM-MEM-001)

Fixed by `kernel/src/memory/address.rs`:

| Constant | Value | Meaning |
|---|---|---|
| `PAGE_SIZE` | 4096 | only page/frame size in v0.1 (§9) |
| `KERNEL_RANGE_START` | `0x8020_0000` | kernel image load base (linker.ld) |
| `KERNEL_RANGE_END` | `0x8800_0000` | end of kernel-reserved RAM (128 MiB QEMU virt) |
| `USER_RANGE_START` | `0x0000_1000` | user window start; page zero never mapped |
| `USER_RANGE_END` | `0x4000_0000` | user window end (exclusive) |

Typed addresses: `VirtAddr` and `PhysAddr` are distinct wrapper types with
no implicit conversion and no arithmetic operators, so virtual and
physical addresses cannot be confused at compile time. The kernel and
user ranges are disjoint by construction (checked by unit test).

## 12. Frame Lifecycle Model (Phase 4, AXIOM-MEM-002)

`kernel/src/memory/frame.rs` realizes §4 as a typed state machine:

```text
Free ──allocate(owner)──> Allocated ──mark_mapped──> Mapped
  ^                          │  ^                      │
  └────────free()────────────┘  └────mark_unmapped────┘
Allocated/Mapped ──quarantine──> Quarantined (terminal this boot)
```

* `FrameOwner` = FreePool | Kernel | AddressSpace(id) — exactly one owner
  at any time (MEM-P4); ownership changes only through `free()`.
* Freeing a mapped frame is rejected (`StillMapped`): no dangling
  mappings can exist in the model.
* `free()` models the mandatory scrub point (no data remanence, §4).
* Quarantined frames reject every operation within the boot session.
* All invalid transitions return explicit `FrameError` values — covered
  by negative unit tests. No allocator and no heap exist in this phase.

## 13. Page Table Model (Phase 4, AXIOM-MEM-003)

`kernel/src/memory/pagetable.rs` realizes §5–§6 as a checked model (no
MMU activation, no satp writes — the hardware Sv39 table added in a later
phase must refine this model). `PageTable::map` validates, in order:
alignment → permission structure → kernel-range/USER exclusion (MEM-P1)
→ virtual-window rules (user pages only in the user window, page zero
never mappable) → frame ownership (MEM-P2) → frame lifecycle (only
`Allocated` frames; a `Mapped` frame can never be mapped a second time,
making sharing structurally impossible) → no double-mapping of a virtual
page → static capacity (`MAX_MAPPINGS` = 64, no heap).

Failure behavior: every rejected rule returns a distinct `MapError` and
leaves both the table and the frame unchanged (mapping is atomic, §1).
`unmap` returns the frame to `Allocated`. Every rule above is covered by
a positive and a negative unit test.
