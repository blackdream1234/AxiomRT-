# AxiomRT

## 1. What is AxiomRT?

AxiomRT is a formally specified microkernel-based safety runtime for
high-assurance embedded systems that require strong isolation, deterministic
execution, controlled fault recovery, and certification-oriented evidence.

The design principle is: **small trusted kernel, isolated user-space
services, explicit authority, controlled failure.**

The first product target is the **AxiomRT Safety Core Industrial Evaluation
Kit** — an evaluation-stage kit for safety-oriented embedded runtime
research and prototype evaluation. It is not sold as a certified OS, and no
certification claim is made.

## 2. What AxiomRT is not

AxiomRT v0.1 is not a Linux clone, not a desktop OS, not a mobile OS, not a
full QNX clone, not a POSIX system, and not a certified product. It is not
aircraft-ready and not vehicle-ready.

v0.1 deliberately excludes: GUI, filesystem, network stack, POSIX layer,
shell, package manager, dynamic kernel modules, user accounts, multicore
support, shared memory IPC, and AI inside the kernel. See
docs/01_SCOPE_AND_NON_GOALS.md.

## 3. Current phase

* **Current milestone:** `v1.3-readonly-fs` — user-space read-only
  filesystem service: `ls`/`cat` in the shell over bounded IPC, zero
  filesystem logic in the kernel (docs/28). Earlier: `v1.2-app-loader`
  (apps by name via user-space loader, docs/27), `v1.1-os-shell`
  (interactive `axiom>`), `v1.0.1-clean`, `v1.0-industrial-eval`.
* **Next milestone:** storage service investigation (QEMU virtio-blk,
  user-space block driver).
* **Next product direction:** real OS completion
  (`AxiomrtFull Completion Mode.md` — user-facing shell, application
  loading, filesystem/storage services, host tooling).
* **Next software phase:** developer tooling (`axiomctl`) + user-facing
  shell.
* **Next hardware phase:** real RISC-V board support (requires a
  physical board with MMU; emulator-only until then).

On QEMU RISC-V 64 the kernel boots through OpenSBI and demonstrates,
each with an automated test: Sv39/MMU hardware memory isolation,
multi-task fixed-priority preemptive scheduling, watchdog containment of
CPU exhaustion, synchronous copy-based IPC, capability enforcement
(deny-by-default), a supervisor/logger fault-recovery chain, and the
full four-task fault-containment demo (a faulty task is contained while
the critical task keeps running and the kernel stays alive). 9/9 QEMU
tests, 129 host tests, and 3 Coq model files pass.

Verify everything: `./scripts/verify_all.sh`. Run the flagship demo:
`cargo build --release --features demo_full && ./scripts/run_qemu.sh`.
Assemble the kit: `./scripts/build_eval_kit.sh`. Read `kit/LIMITATIONS.md`
and `kit/ASSUMPTIONS_OF_USE.md` first — this is an emulator-only,
evaluation-stage kit with no certification claim. Roadmap beyond v1.0
(real hardware, pilots, certification) needs a physical board and
external parties and is out of scope for this kit.

## 4. Target platform

* Architecture: RISC-V 64 (RV64GC)
* Platform: QEMU `virt` machine (emulator only in v0.1)
* Boot firmware: OpenSBI
* Kernel language: Rust `no_std`, minimal RISC-V assembly
* Deployment target v0.1: emulator only

## 5. Architecture direction

Microkernel architecture. The kernel provides only controlled mechanisms:
boot, traps, interrupts, address spaces, threads, fixed-priority scheduling,
synchronous copy-based IPC, capability-based access control, syscall
validation, and fault events. Everything complex — drivers, logging, health
monitoring, supervision policy — runs isolated in user space.

```text
+--------------------------------------------------+
|                User-Space Services               |
| supervisor | logger | critical task | faulty task|
+--------------------------------------------------+
                    | syscalls / IPC
+--------------------------------------------------+
|                   AxiomRT Kernel                 |
| traps | syscalls | scheduler | IPC | capabilities|
| address spaces | threads | faults | timers       |
+--------------------------------------------------+
|             RISC-V 64 / QEMU / OpenSBI           |
+--------------------------------------------------+
```

## 6. Safety rule

Every line of code must be traceable to a requirement, a design decision, a
safety rule, a test, and a verification objective. If a feature cannot be
traced, it does not belong in the system.

Core v0.1 guarantees: memory isolation, capability-controlled access,
deterministic scheduling, and controlled user-space fault recovery. A user
task fault must never crash the kernel.

## 7. Codex rule

No code before the corresponding document exists. AI assistants are
implementation assistants only: they execute precisely scoped tasks (Task
ID, allowed files, forbidden files, definition of done) and never invent
architecture, add dependencies, remove tests, or weaken checks. Full rules:
docs/07_CODEX_RULES.md.

## 8. Phase map

```text
Phase 0:  Kernel Blueprint            (complete — gated)
Phase 1:  Repository Skeleton         (complete)
Phase 2:  Boot Kernel in QEMU         (complete)
Phase 3:  Trap and Exception Layer    (complete)
Phase 4:  Memory Isolation            (complete — MMU on target, v0.2)
Phase 5:  Thread Model                (complete)
Phase 6:  Scheduler                   (complete — preemptive on target, v0.4)
Phase 7:  User Mode                   (complete)
Phase 8:  IPC                         (complete — on target, v0.6)
Phase 9:  Capabilities                (complete — on target, v0.7)
Phase 10: Fault Recovery              (complete — supervisor chain, v0.8)
Phase 11: Runtime Monitoring          (complete)
Phase 12: Formal Proof Starter        (complete — model-level, refinement TODO)
Phase 13: Industrial Evaluation Kit   (complete — v1.0)
```

Next: the real-OS completion phases (developer CLI, structured events,
Studio dashboard, installer, CI, init/console/shell services,
application loading, filesystem/storage, drivers, real hardware) are
defined in `AxiomrtFull Completion Mode.md`.

Documentation index: docs/INDEX.md. Start at docs/00_PROJECT_CHARTER.md.
