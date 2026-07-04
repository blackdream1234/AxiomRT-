# AxiomRT Trap Model

Document ID: created by AXIOM-TRAP-001 (Phase 3)
Requirement reference: docs/02_KERNEL_BLUEPRINT.md §3 (trap handling),
docs/06_FAULT_MODEL.md.

## 1. Principle

Every entry into the kernel goes through one controlled trap path. There
are no side doors: exceptions, interrupts, and syscalls all enter at the
supervisor trap vector, save full register state, run a Rust handler, and
leave through a single restore path (`sret`). No trap is ever silently
ignored; unknown traps are a controlled panic, never undefined behavior.

## 2. Trap Vector (AXIOM-TRAP-001)

* `stvec` is set once at boot (`trap::init()`), direct mode, pointing to
  `__trap_vector` in `kernel/src/arch/riscv64/trap.S`.
* `__trap_vector` pushes a 256-byte trap frame on the kernel stack:
  general registers `x1..x31` plus `sepc`. Layout is a fixed `#[repr(C)]`
  contract with `TrapFrame` in `trap.rs`.
* Phase 3 assumption (documented, revisited in Phase 7): traps originate
  from kernel (S-mode) context only, so the frame lives on the current
  kernel stack. User-mode trap entry with stack switching arrives with
  user mode (docs/10_USER_MODE.md, Phase 7).
* After the handler returns, `sepc` is written back (handlers may advance
  it), registers are restored, and `sret` resumes execution.

## 3. Cause Decoding

The Rust handler `trap_handler(&mut TrapFrame)` reads `scause` and
decodes:

| scause | Meaning | Phase 3 behavior |
|---|---|---|
| interrupt bit set | any interrupt | controlled panic (no interrupt sources are enabled in Phase 3) |
| 2 | illegal instruction | structured trap message, safe halt (AXIOM-TRAP-002) |
| 8 | ecall from U-mode | syscall stub dispatch (AXIOM-TRAP-003) |
| 9 | ecall from S-mode | never delegated here — OpenSBI owns it (see §5); if it arrives, controlled panic |
| other | unknown trap | structured trap message, controlled panic |

## 4. Structured Trap Messages (AXIOM-TRAP-002)

Every abnormal trap prints one structured line over serial before any
halt, so behavior is observable and testable:

```text
TRAP kind=<name> cause=<hex> sepc=<hex> stval=<hex>
```

followed by a terminal status line, either:

```text
HALT reason=illegal_instruction phase=trap
PANIC kernel=axiomrt reason=<unknown_trap|unexpected_interrupt> phase=trap
```

There is no undefined trap behavior: every reachable `scause` value maps
to exactly one of the rows in section 3.

## 5. Syscall Trap Stub (AXIOM-TRAP-003)

* An `ecall` trap from **user mode** (scause 8) is the syscall path. The
  syscall number is taken from `a7`, the result is returned in `a0`
  (docs/04_SYSCALL_MODEL.md ABI), and `sepc` is advanced by 4 so the
  `ecall` is never re-executed.
* Dispatch goes to `kernel/src/syscall/mod.rs`. Phase 3 is a stub layer:
  known syscall numbers are acknowledged with a structured serial line and
  return `ERR_NOT_IMPLEMENTED`; unknown numbers log a controlled error and
  return `ERR_INVALID_SYSCALL`. No syscall logic beyond stub dispatch
  exists (no IPC, no capabilities — Phase 3 boundary).
* **Platform fact (verified on QEMU virt + OpenSBI):** `ecall` from
  S-mode is the SBI call convention. It traps to machine mode and is
  handled by OpenSBI; `medeleg` does not delegate cause 9 to the kernel
  (observed `medeleg=0xf4b509`: bit 8 set, bit 9 clear). The kernel
  therefore cannot invoke its own syscall path from kernel context, and
  an S-mode ecall arriving at `stvec` can only mean broken delegation —
  it is treated as a controlled panic.
* Consequence for verification: the trap entry/exit machinery is proven
  in Phase 3 by the illegal-instruction path; `syscall::dispatch` is
  verified by direct invocation. The full U-mode `ecall` → trap →
  dispatch → `sret` round trip becomes testable in Phase 7 (user mode),
  where the IllegalSyscall fault path of docs/06_FAULT_MODEL.md applies
  to the calling task.

## 6. Fault Model Hook

Phase 3 halts are placeholders for the fault model: once threads exist
(Phase 5) and fault events exist (Phase 10), user-originated traps stop
halting the system and instead mark the offending thread Faulted and
notify the supervisor. Kernel-originated unknown traps remain
KernelInvariantViolation → controlled panic (docs/06_FAULT_MODEL.md).
