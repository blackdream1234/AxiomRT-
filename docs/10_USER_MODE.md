# AxiomRT User Mode

Document ID: created by AXIOM-USER-002 (Phase 7)
Requirement reference: docs/02_KERNEL_BLUEPRINT.md §7 (trust boundary),
docs/10_TRAP_MODEL.md, docs/06_FAULT_MODEL.md.

(Naming note: the file number 10 collides with 10_TRAP_MODEL.md; both
names are fixed verbatim by the task pack.)

## 1. Principle

User tasks run at RISC-V privilege U. The kernel/user boundary is
crossed in exactly two directions:

* **down (S→U):** one controlled transition, `__enter_user`
  (`kernel/src/arch/riscv64/user_entry.S`);
* **up (U→S):** only through the trap vector — syscall (`ecall`),
  exception, or (later) interrupt.

There is no other path. A "bad return path" cannot exist structurally:
`__enter_user` never returns and leaves no return address; the only way
back is `__trap_vector`.

## 2. The S→U Transition (`__enter_user`)

```text
__enter_user(entry, user_sp, trap_stack_top):
    sscratch := trap_stack_top   # arm the trap vector for user re-entry
    sepc     := entry
    sstatus.SPP := 0             # sret drops to U
    sp       := user_sp
    sret
```

Caller obligations (enforced in `kernel/src/user/mod.rs`): valid entry,
valid stacks, and a registered fault continuation (§4) *before* the
transition.

## 3. sscratch Discipline (trap re-entry)

* In kernel context: `sscratch = 0`.
* In user context: `sscratch = kernel trap stack top`.

`__trap_vector` (trap.S) swaps `sp` with `sscratch`: zero → the trap
came from kernel code (keep the current stack); nonzero → the trap came
from user code (switch to the kernel trap stack, record the user sp in
the frame, set `sscratch = 0` while handling). On `sret` back to user,
`sscratch` is re-armed with the trap stack top. The trap frame carries
`sstatus`, so the pre-trap privilege (SPP) is part of the saved state
and handlers decide user vs. kernel policy from the frame, not from
globals.

## 4. User Fault Containment

Before entering user mode, the kernel registers a continuation
(resume PC + kernel stack) with the trap layer
(`trap::set_user_fault_continuation`). Any non-syscall exception from
user mode then:

1. prints the structured `TRAP` line (docs/10_TRAP_MODEL.md §4),
2. prints `CONTAIN scope=user reason=<...> action=terminate_task
   kernel=alive`,
3. rewrites the trap frame: `sepc := continuation`, destination stack
   `:= kernel stack`, `sstatus.SPP := 1`,
4. `sret` resumes the *kernel*, not the faulted task. The task never
   executes again (docs/06_FAULT_MODEL.md: Faulted is terminal;
   Phase 10 turns this hard-coded termination into supervisor policy).

A user fault therefore cannot crash or wedge the kernel. Kernel-context
faults keep their Phase 3 behavior: controlled halt.

## 5. Phase 7 Boundary and Limitations (explicit)

* **Privilege isolation is active:** the user task cannot execute
  privileged instructions or CSR accesses — attempting one traps
  (verified: `csrr sstatus` from U raises IllegalInstruction).
* **Memory isolation is NOT yet hardware-enforced:** the MMU is not
  activated (satp=Bare); the demo user task executes from kernel RAM
  and its stack lives in kernel .bss. The memory model of Phase 4 is
  the specification the MMU activation phase will enforce. Until then,
  claims about memory isolation are model-level only
  (docs/05_MEMORY_MODEL.md).
* The demo validates the v0.1 *virtual* user layout through the
  `UserImage` model (entry `0x1_0000`, stack top `0x20_0000`,
  docs/05 §11) even though pre-MMU execution uses physical addresses.
* One user task, no scheduler dispatch integration, no IPC, no
  capabilities (Phases 8–10).

## 6. Verified Behavior (QEMU, AXIOM-USER-002 definition of done)

Observed serial output after the boot banner:

```text
USER enter=demo_task mode=U isolation=privilege
SYSCALL name=sys_yield status=stub result=ERR_NOT_IMPLEMENTED
SYSCALL name=sys_exit status=stub result=ERR_NOT_IMPLEMENTED
TRAP kind=illegal-instruction cause=0x...2 sepc=0x... stval=0x...
CONTAIN scope=user reason=illegal_instruction action=terminate_task kernel=alive
USER demo=first_user_task result=contained kernel=survived
phase=user-demo-complete
```

This demonstrates: (a) the first user task runs in U-mode, (b) it traps
back through the syscall path and resumes, (c) a deliberate fault is
contained, and (d) the kernel survives.
