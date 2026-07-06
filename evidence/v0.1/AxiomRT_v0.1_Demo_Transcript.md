# AxiomRT v0.1 Demo Transcript

Captured serial transcript of the AxiomRT v0.1 fault-containment demo on
QEMU. Source log: `qemu_demo.log` (this directory).

Requirement reference: Full Completion Mode §10, docs/DEMO_SCENARIO.md.

## Command

```sh
qemu-system-riscv64 -machine virt -smp 1 -m 128M -nographic \
  -bios default -kernel target/riscv64gc-unknown-none-elf/release/kernel
```

(The kernel halts in a `wfi` loop after the demo, so the run is bounded
by a timeout; that is the expected exit.)

## Transcript (AxiomRT lines; OpenSBI banner omitted)

```text
AxiomRT kernel booted
arch=riscv64
phase=boot
USER enter=demo_task mode=U isolation=privilege
SYSCALL name=sys_yield status=stub result=ERR_NOT_IMPLEMENTED
SYSCALL name=sys_exit status=stub result=ERR_NOT_IMPLEMENTED
TRAP kind=illegal-instruction cause=0x0000000000000002 sepc=0x0000000080200fe6 stval=0x00000000100022f3
CONTAIN scope=user reason=illegal_instruction action=terminate_task kernel=alive
USER demo=first_user_task result=contained kernel=survived
phase=user-demo-complete
```

## Line-by-line meaning

| Line | Meaning |
|---|---|
| `AxiomRT kernel booted` / `arch` / `phase=boot` | boot banner (checked by the smoke test) |
| `USER enter=demo_task mode=U isolation=privilege` | kernel dropped to user privilege (U-mode) |
| `SYSCALL name=sys_yield …` | full U-mode → trap → dispatch → U-mode round trip |
| `SYSCALL name=sys_exit …` | second syscall round trip |
| `TRAP kind=illegal-instruction …` | user task executed a privileged instruction; hardware trapped it |
| `CONTAIN … kernel=alive` | kernel terminated the faulting task and kept running |
| `USER demo=… kernel=survived` / `phase=user-demo-complete` | kernel continuation confirms survival |

## What this demonstrates

1. The kernel/user privilege boundary is real (U-mode entry).
2. Syscalls traverse the controlled trap path and return.
3. A user fault is contained; the kernel survives (a user fault cannot
   crash the kernel).
4. All evidence is structured and parseable on the serial port.

Boundary reminder: v0.1 demonstrates privilege isolation and fault
containment; memory isolation via MMU arrives in v0.2 (see the final
report §5).
