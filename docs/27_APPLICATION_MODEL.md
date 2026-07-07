# 27 — Application Model

Document ID: created by AXIOM-APP-001 (Real OS application phase).
Requirement reference: `AXIOMapp.md`, `AxiomrtFull Completion Mode.md`
§17, docs/25_OS_BOOT_FLOW.md, docs/26_SHELL.md.

## 1. Services vs applications

* **Services** are the OS itself: init, supervisor, logger, console,
  shell, app_loader. Started by init's boot policy, expected to run
  forever, hold infrastructure capabilities.
* **Applications** are workloads a user starts by name from the shell.
  They run isolated in their own address space, hold only the
  capabilities their manifest grants (deny-by-default), may exit, may
  fault, and may be re-run. An application fault never harms the
  kernel, the services, or another application.

Mechanism/policy split (unchanged Architecture Law): the kernel only
creates address spaces from static images, maps code/rodata/stack,
assigns manifest capabilities, starts/kills/restarts tasks, and
contains faults. **All app policy — which names exist, what may be
started, what to answer the shell — lives in `app_loader_service`**
(user space), with the shell as the human surface and init as boot
policy.

## 2. Current stage: static app table

Applications are built into the kernel image's `.user` region exactly
like services (docs/25 §2 constrained-Rust rules) and described by two
tables:

* **Kernel service table** (mechanism): entry address, stack page,
  priority, TCB slot, capability grants. The kernel does not know
  which entries are "apps".
* **Loader manifest** (policy, in app_loader's `.user.rodata`): app
  name → kernel table index, plus the human description served by
  `app info`.

An app slot whose task has Exited/Faulted/been Killed is re-armed on
the next `run <name>`: the kernel resets its initial frame (same
entry, same stack, same capabilities). Slots in Ready/Running/Blocked
state are never restarted implicitly (`already running` error).

## 3. Future stage (explicitly not implemented here)

Position-independent app images and a restricted ELF loader, loaded
from the filesystem service once it exists, with per-image manifests
and signature checks later. **This phase adds no filesystem, no
storage, no dynamic loading from disk.**

## 4. Manifest fields

Per application: `name`, `entry point`, `priority`, `stack` (one 4 KiB
page in this stage), `allowed capabilities`, `restart policy`
(`rerun-on-request`; the supervisor's fault policy remains Kill),
`description`.

## 5. Built-in applications and capability grants

| app | prio | capabilities | behavior |
|---|---|---|---|
| hello | 2 | console write | prints `hello from app: hello`, exits cleanly |
| fault_demo | 2 | **none** | attempts an unauthorized console write (CAP_DENIED), then a CPU-exhaustion loop; watchdog contains it, supervisor kills it, shell stays alive |
| counter | 2 | console write | emits `APP counter progress=1..3` with yields between, exits cleanly |

Apps run at priority 2 (below the shell's 3, above the console idle
poller's 1): they execute when the shell is waiting for input and can
never starve the operator surface.

## 6. Application lifecycle

`Available` (manifest entry, slot Empty or terminated) → `Loaded/
Running` (`run <name>`: address space built or frame re-armed, task
Ready) → `Exited` (sys_exit) / `Faulted` (contained; supervisor
policy applies) / `Killed` (`kill <idx>` or supervisor Kill). A
terminated app returns to `Available` and can be re-run.

## 7. Shell commands

New: `apps` (list names), `app info <name>` (one-line manifest
summary), `run <name>` (start by name). Existing commands are
preserved unchanged, including `run demo`, `kill <idx>`,
`restart <idx>`. The shell forwards the raw `apps` / `app info …` /
`run <name>` line to app_loader over the bounded app endpoint
(endpoint 0, ≤ 64 bytes) and prints the loader's one-line reply; the
shell itself contains no app-name knowledge.

## 8. Security rules

1. Deny-by-default: an app holds exactly its manifest capabilities;
   fault_demo holds none and its first act is denied and logged.
2. Only capability holders reach app control: the loader holds the
   task-control capability to request starts; the shell keeps its own
   for kill/restart/shutdown. No app holds control or info authority.
3. App I/O buffers pass the same validated, SUM-gated copy paths as
   everything else; app faults go through the same containment +
   supervisor notification chain (docs/26 §3).
4. The kernel never parses app names — names are loader policy.

## 9. Limitations

* Apps are compiled into the kernel image (static table stage); no
  dynamic loading, no external images yet.
* One 4 KiB stack page per app; message and reply lines ≤ 64 bytes.
* The loader's control capability is broader than "start-only"; a
  narrower spawn right is future work, stated here rather than hidden.
* `restart <idx>` addresses TCB slots (kernel view), while `run
  <name>` addresses manifest names (loader view).
* Emulator-only, evaluation stage, no certification claim.
