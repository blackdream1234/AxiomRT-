# AxiomRT Sv39 / MMU Hardware Memory Isolation

Document ID: created by AXIOM-MEMHW-001 (v0.2, Stage 1)
Requirement reference: docs/05_MEMORY_MODEL.md, Full Completion Mode
§11. This document upgrades the memory-isolation claim from model-level
(v0.1) to hardware-enforced on QEMU for the tested cases.

## 1. Goal and Boundary

v0.2 activates the RISC-V Sv39 MMU so the hardware enforces the memory
model of docs/05_MEMORY_MODEL.md. Scope of this stage: kernel page
table, satp activation, one user address space, U-mode entry under that
address space, page-fault containment, and negative tests. **Not** in
this stage: scheduler dispatch, IPC, supervisor recovery, timer,
drivers, multicore (those are later stages).

The claim after v0.2: for the tested cases (user read of kernel memory,
user write of unmapped memory, user execute of a non-executable page)
memory isolation is enforced by the MMU on QEMU and the resulting page
fault is contained. Untested access patterns remain a documented gap.

## 2. Sv39 Address Translation

* 39-bit virtual addresses, 3 levels of 512-entry page tables, 4 KiB
  pages (v0.1/v0.2: 4 KiB only, no megapages).
* `satp` register: MODE=8 (Sv39), ASID=0 (single address space active
  at a time in v0.2), PPN = physical page number of the root table.
* Virtual address split: `[38:30] VPN2 | [29:21] VPN1 | [20:12] VPN0 |
  [11:0] offset`.

## 3. Page Table Entry (PTE) Format

64-bit PTE (`kernel/src/arch/riscv64/sv39.rs`, AXIOM-MEMHW-002):

```text
bit  0  V  valid
bit  1  R  read
bit  2  W  write
bit  3  X  execute
bit  4  U  user-accessible
bit  5  G  global
bit  6  A  accessed
bit  7  D  dirty
bits 8-9    RSW (reserved for software)
bits 10-53  PPN (physical page number, 44 bits)
```

Encoding rules enforced by the constructor (refines
docs/05_MEMORY_MODEL.md §6):

* A leaf PTE has at least one of R/W/X set; a pointer (non-leaf) PTE has
  R=W=X=0 and V=1.
* `W` without `R` is reserved/illegal — rejected.
* Kernel mappings set `U=0`; user mappings set `U=1`. The two are never
  combined (USER/KERNEL mutual exclusion).
* User mappings are W^X: never W and X together (MEM-P5).
* A/D are pre-set for mapped leaves to avoid relying on hardware A/D
  update (QEMU virt supports it, but pre-setting keeps behavior
  deterministic).

## 4. Kernel Page Table (AXIOM-MEMHW-003, -004)

The kernel builds a static root page table at boot mapping exactly the
regions it needs, all with `U=0`:

| Region | Perms | Purpose |
|---|---|---|
| kernel text | R,X | code |
| kernel rodata | R | constants |
| kernel data/bss/stack | R,W | data + boot/trap stacks |
| UART MMIO 0x1000_0000 | R,W (no X) | serial device (kernel-only) |

Identity mapping (VA=PA) is used in v0.2 so that pointers valid before
activation stay valid after — the transition does not relocate the
kernel. `satp` is written and an `sfence.vma` issued (AXIOM-MEMHW-004);
after this point every kernel access is translated, and no kernel
mapping carries the U bit.

## 5. User Address Space (AXIOM-MEMHW-005, -006, -007)

Each user task gets its own root table (AXIOM-MEMHW-005) mapping only
its own regions, all with `U=1`:

| Region | Perms |
|---|---|
| user code | R,X (U) |
| user rodata | R (U) |
| user data/stack | R,W (U) |

No kernel region is mapped into the user table (no U on kernel frames —
the model already rejects it structurally, docs/05 §5). Entering U-mode
(AXIOM-MEMHW-007) switches `satp` to the user root table before `sret`,
so the user task runs under hardware-enforced isolation.

## 6. Page Fault Handling (AXIOM-MEMHW-008)

Sv39 raises three fault causes the kernel decodes:

* scause 12 — instruction page fault (bad fetch)
* scause 13 — load page fault (bad read)
* scause 15 — store/AMO page fault (bad write)

From user mode these are contained exactly like the illegal-instruction
path of docs/10_USER_MODE.md §4: structured `TRAP kind=page-fault`
report, `CONTAIN scope=user reason=page_fault`, task terminated, kernel
survives. `stval` carries the faulting address; it is included in the
event. A page fault taken in kernel mode remains a
KernelInvariantViolation → controlled halt (docs/06_FAULT_MODEL.md).

## 7. Negative Tests (AXIOM-MEMHW-009, -010, -011)

On-target QEMU tests drive a user task that attempts a forbidden access
and assert containment:

* `user_read_kernel` — load from a kernel address → load page fault,
  contained.
* `user_write_unmapped` — store to an unmapped user address → store
  page fault, contained.
* `user_exec_nonexec` — jump into a non-executable page → instruction
  page fault, contained.

Each asserts the kernel prints the containment line and survives.

## 8. Verification Impact (AXIOM-MEMHW-012)

proofs/coq/MemoryIsolation.v gains an explicit refinement note: the
Sv39 leaf-PTE encoding realizes the `AddressSpace` model map — a leaf
PTE with `U=1` exists in a user table iff the model has a user mapping
for that page, and no kernel frame ever carries `U=1`. The refinement
theorem statement is added (still an explicit TODO for the full proof),
connecting MEM-P1 to the concrete PTE encoding.

## 9. Expected QEMU Output

```text
AxiomRT kernel booted
arch=riscv64
phase=boot
MMU status=enabled mode=sv39 scope=kernel
USER enter=demo_task mode=U isolation=memory
TRAP kind=page-fault reason=user_access_kernel_memory
CONTAIN scope=user reason=page_fault action=terminate_task kernel=alive
USER demo=memory_isolation result=contained kernel=survived
```
