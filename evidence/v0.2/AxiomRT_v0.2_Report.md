# AxiomRT v0.2 Report — Sv39/MMU Hardware Memory Isolation

Requirement reference: Full Completion Mode §11 (Stage 1),
docs/12_MMU_SV39.md.

## 1. Goal Achieved

v0.2 activates the RISC-V Sv39 MMU and enforces memory isolation on
QEMU for the tested cases, upgrading the memory-isolation claim from
model-level (v0.1) to hardware-enforced.

**No certification claim. No production-readiness claim.**

## 2. What Changed Since v0.1

* Sv39 PTE encoding (`kernel/src/arch/riscv64/sv39.rs`) — rejects every
  illegal permission combination by construction.
* Index-based Sv39 page table builder with a `translate` mirror
  (`paging.rs`) — host-tested mapping logic.
* Kernel identity page table + `satp` activation (`paging_hw.rs`,
  linker section symbols) — kernel mappings carry no U bit.
* Per-task user address space (kernel maps U=0 for the trap handler +
  user code/stack U=1); demo task runs under its own page table.
* Page-fault decoding in the trap layer with classified reasons.
* Three QEMU negative tests; Coq Sv39 encoding refinement lemmas.

## 3. Verified Facts

| Fact | Evidence |
|---|---|
| Sv39 enabled at boot | `qemu_mmu_demo.log` (`MMU status=enabled mode=sv39`) |
| User read of kernel memory → contained page fault | `memory_isolation.log` (read-kernel) |
| User write of unmapped address → contained page fault | `memory_isolation.log` (write-unmapped) |
| User execute of non-exec page → contained page fault | `memory_isolation.log` (exec-nonexec) |
| Boot smoke test passes | `boot_smoke.log` |
| Host test suites pass (109) | `host_tests.log` |
| MemoryIsolation.v compiles (with Sv39 lemmas) | `coq_memory.log` |

Observed containment (read-kernel case): task runs at user VA
`0x00010fe0` (its own mapping), reads kernel `0x80200000` (stval),
takes a load page fault (cause 0x0d), is contained
(`reason=user_access_kernel_memory`), kernel survives.

## 4. Boundary and Limitations (explicit)

* Memory isolation is MMU-enforced **for the tested cases**; untested
  access patterns remain a documented gap.
* Still one user task on target; multi-task dispatch, timer preemption,
  watchdog, on-target IPC, and the supervisor chain remain host-tested
  models (Stages 2–7).
* 4 KiB pages only; no megapages; single hart; single active ASID.
* The demo user task's code/stack frames are physically inside kernel
  RAM and exposed at user virtual addresses (no separate user binary
  loader yet) — a real deployment loads a distinct user image into
  dedicated frames.
* Coq: model + Sv39 encoding lemmas proven; full Rust refinement TODO.

## 5. Next Stage

v0.3 — On-Target Multi-Task Dispatch (Full Completion Mode §12).
