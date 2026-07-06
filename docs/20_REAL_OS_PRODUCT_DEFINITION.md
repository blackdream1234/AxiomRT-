# 20 — AxiomRT Real OS Product Definition

Document ID: created by AXIOM-PRODUCT-001.
Requirement reference: `AxiomrtFull Completion Mode.md` §1 (Final Goal),
§3 (Architecture Law), §4 (User-Space OS Services), §5 (Host-Side
Tools), §28 (Final Definition of Done); docs/00_PROJECT_CHARTER.md.

## 1. What AxiomRT Real OS Is

AxiomRT Real OS is the completion target of the AxiomRT project: a
real, runnable, user-facing operating system for high-assurance
embedded systems, built on the existing formally specified microkernel.
"Real" means a person — not the author — can install it, boot it (in
QEMU and on at least one physical RISC-V board with MMU), get an
interactive shell, run isolated applications, read files from a
filesystem service, inspect faults and capabilities, and reproduce
every piece of verification evidence with one command.

It keeps the founding principle unchanged: **small trusted kernel,
isolated user-space services, explicit authority, controlled failure.**

## 2. What Is Different from the Evaluation Kit

The v1.0 Industrial Evaluation Kit (tag `v1.0-industrial-eval`)
demonstrates *mechanisms*: isolation, scheduling, watchdog, IPC,
capabilities, supervisor recovery — each proven by a scripted QEMU demo
with serial assertions. It has no interactive surface: every scenario
is compiled in via a cargo feature and observed on the serial port.

The Real OS adds the *operating system* around those mechanisms:

| Aspect | Evaluation kit (v1.0) | Real OS |
|---|---|---|
| User interface | none (serial log only) | interactive shell (`axiom>`) via console service |
| Task origin | compiled-in demo tasks | `init_service` boot policy + app loader |
| Applications | none | isolated user apps started/stopped from shell |
| Files | none | user-space read-only FS service, then writable |
| Storage | none | user-space block service (QEMU virtio-blk first) |
| Drivers | none (demo mechanisms only) | user-space driver framework with restart |
| Host tooling | shell scripts | `axiomctl` CLI + AxiomRT Studio dashboard |
| Setup | manual toolchain steps | `./install.sh` one command |
| CI | none | GitHub Actions (build, tests, QEMU, Coq, clippy, fmt) |
| Platform | QEMU only | QEMU **and** one real RISC-V board |

What does **not** change: the microkernel boundary (§4), the
document-first Codex rules, one commit per task, honest limitations,
and the absence of any certification claim.

## 3. Final Real OS Architecture

```text
+------------------------------------------------------------------+
|                        Host-side tooling                         |
|   axiomctl (CLI)   |   AxiomRT Studio (local dashboard)  | CI    |
+------------------------------------------------------------------+
                 (builds / runs / parses serial evidence)
+------------------------------------------------------------------+
|                     User space (isolated services)               |
| init | supervisor | logger | console | shell | fs | storage |    |
| driver_manager | app_loader | health_monitor | user apps ...     |
+------------------------------------------------------------------+
                          | syscalls / bounded IPC / capabilities
+------------------------------------------------------------------+
|                      AxiomRT microkernel                          |
| boot | traps | interrupts | address spaces | page tables |        |
| threads | scheduler | IPC | capability lookup | syscall           |
| validation | timer | fault events | minimal HAL                   |
+------------------------------------------------------------------+
|        RISC-V 64 (RV64GC, Sv39) — QEMU virt / real board          |
|                          OpenSBI firmware                         |
+------------------------------------------------------------------+
```

## 4. Kernel Responsibilities (unchanged Architecture Law)

The kernel may contain only: boot, trap handling, interrupt routing,
address spaces, page tables, thread/process model, scheduler, IPC
mechanism, capability lookup, syscall validation, timer mechanism,
fault event creation, and the minimal hardware abstraction required
for boot and isolation.

The kernel must never contain: GUI, filesystem logic, network stack,
shell, package manager, complex drivers, dynamic policy, AI logic,
user accounts, high-level logging backends, or application frameworks.
Everything complex runs in isolated user space. Any task that would
violate this law is rejected regardless of convenience.

## 5. User-Space Service Responsibilities

1. **init_service** — first user-space service; owns boot policy;
   starts and orders all other services.
2. **supervisor_service** — receives fault events over
   capability-checked IPC; applies recovery policy (kill / restart /
   suspend decisions).
3. **logger_service** — receives structured kernel/user events;
   exports serial logs and evidence logs.
4. **console_service** — owns console input/output; exposes the
   terminal to the shell.
5. **shell_service** — interactive user shell; inspects tasks, memory,
   capabilities, events, services; starts/stops apps.
6. **fs_service** — user-space filesystem; read-only embedded archive
   first, writable storage later.
7. **storage_service** — user-space block abstraction; QEMU virtio-blk
   first, hardware storage later.
8. **driver_manager** — starts user-space drivers, grants device
   capabilities, restarts failed drivers.
9. **app_loader** — loads static applications first, then a restricted
   executable format.
10. **health_monitor** — heartbeat monitoring; health state exposed to
    shell and dashboard.

Each service is a separate address space with only the capabilities its
manifest grants. A service crash is a contained fault, never a kernel
crash.

## 6. Host Tooling Responsibilities

1. **axiomctl** — developer CLI: `doctor`, `build`, `run`,
   `demo memory|full`, `verify`, `evidence list|open`, `kit build`,
   `release check`. Wraps the existing scripts; never bypasses them.
2. **AxiomRT Studio** — local graphical dashboard: run the demo, view
   the event timeline, task/fault/IPC/capability tables, test and Coq
   status, evidence archives, limitations, release builder. Host-side
   only; talks to the system exclusively through `axiomctl` and parsed
   serial evidence. No GUI code in or near the kernel.
3. **install.sh** — one-command setup: detect distro, check/install
   Rust + target + QEMU + (optionally) Coq, build, run the boot smoke
   test, print next commands.
4. **GitHub Actions CI** — build, fmt, clippy `-D warnings`, host
   tests, QEMU tests, Coq compilation, release packaging on every push.

## 7. Hardware Targets

* **Primary (always):** QEMU `virt`, RV64GC, Sv39, OpenSBI.
* **Required for completion:** at least one physical RISC-V board with
  MMU (Sv39) support — candidate class: StarFive VisionFive 2 /
  SiFive HiFive Unmatched / equivalent. Board selection is its own
  documented task (AXIOM-HW-001) with reasons.
* Single hart remains the supported configuration until a dedicated
  multicore phase exists (none is planned in this roadmap).

Hardware tasks require the physical board to be present. If no board is
available, the hardware phase is recorded as a blocker document
(`docs/blockers/`) — it is never simulated, faked, or claimed.

## 8. User Experience

A user of the finished Real OS can:

1. Run `./install.sh` on a fresh Linux machine and get a working
   toolchain plus a passing boot smoke test.
2. Run `axiomctl demo full` and watch the fault-containment demo.
3. Boot to an interactive `axiom>` prompt (QEMU or board).
4. Use shell commands: `help`, `version`, `tasks`, `faults`, `ipc`,
   `caps`, `memory`, `uptime`, `events`, `run <app>`, `kill <task>`,
   `restart <task>`, `ls`, `cat`, `clear`, `shutdown`.
5. Run isolated applications (`run hello`, `run fault_demo`) and watch
   a faulty app be contained while the system keeps running.
6. Open AxiomRT Studio and see the same behavior visually.

## 9. Developer Experience

A developer can:

1. Clone, `./install.sh`, `axiomctl doctor` — environment verified.
2. `axiomctl build && axiomctl run` — kernel in QEMU.
3. `axiomctl verify` — the full 9+ QEMU / host / Coq sweep, identical
   to CI.
4. `axiomctl evidence list` — inspect archived per-version evidence.
5. Read `docs/` phase documents that match the code (document-first
   rule); every file traces to a Phase 0+ document, every commit to a
   task ID.
6. Add an application under `userland/apps/` with a capability
   manifest, without touching the kernel.

## 10. Final Definition of Done

The Real OS is complete only when all 38 conditions of
`AxiomrtFull Completion Mode.md` §28 hold — abbreviated: boots in QEMU
**and** on real hardware with MMU; multiple isolated U-mode processes;
preemption, watchdog, IPC, capabilities, fault containment, supervisor,
logger, init, console, shell, app loading, fs service, storage service
(QEMU at minimum), driver framework; all existing demos and tests still
pass; refinement boundaries explicit; fuzzing campaign exists; CI
green; installer, axiomctl, and Studio work; user and developer docs
exist; evidence and release packages exist; limitations honest; no
certification claim; an external reviewer has actually run the system.

Only then may it be called **AxiomRT Real OS Complete Edition**.

## 11. Explicit Non-Goals

* POSIX compatibility, Linux ABI, or a general-purpose desktop/mobile OS.
* Multicore/SMP scheduling.
* Networking beyond a minimal QEMU virtio-net experiment (Phase 13, and
  only after storage and shell are stable).
* In-kernel anything from the forbidden list (§4).
* Dynamic kernel modules, package manager, user accounts.
* Shared-memory IPC (bounded copy-based IPC remains the only channel).
* AI components anywhere in the TCB.
* Performance competition with commercial RTOSes; determinism and
  auditability win over throughput.

## 12. Certification Boundary

Four tiers, never to be conflated:

1. **Evaluation kit (v1.0, exists):** mechanisms demonstrated on QEMU
   with archived evidence. No fitness-for-use claim.
2. **Real OS (this roadmap's target):** installable, interactive,
   hardware-booting OS with reproducible verification and honest
   limitations. Still **not** a safety-certified product; suitable for
   research, prototyping, and evaluation.
3. **Certification-ready product (out of scope here):** Real OS plus a
   complete safety lifecycle (hazard analysis, FMEA/FTA, traceability
   matrix, safety manual, independent assessment readiness) executed
   with external safety engineers against a specific standard (e.g.
   IEC 61508, ISO 26262, DO-178C) and a specific item definition.
4. **Certified product (out of scope, external):** tier 3 assessed and
   certified by an accredited body for a concrete system context.
   Certification applies to a system in context, never to a kernel in
   isolation.

This project may claim tier 1 today and tier 2 when the §28 gate
passes. Tiers 3 and 4 are never claimed by this repository. Any
wording in code, docs, marketing, or reports that blurs these tiers is
a defect and must be fixed like one.
