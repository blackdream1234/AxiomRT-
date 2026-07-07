# 25 — Real OS Boot Flow and init_service

Document ID: created by AXIOM-INIT-001 (Real OS Phase 7).
Requirement reference: `AxiomrtFull Completion Mode.md` §15,
docs/20_REAL_OS_PRODUCT_DEFINITION.md §5, docs/13_DISPATCH.md,
docs/04_SYSCALL_MODEL.md.

## 1. Boot order

Selected by the `os_boot` cargo feature (the default build and all
demo features are unchanged):

1. OpenSBI (M-mode firmware).
2. AxiomRT kernel entry at 0x8020_0000 (S-mode).
3. Kernel initializes memory: Sv39 kernel identity map, MMU on.
4. Kernel initializes the timer (SBI TIME, preemption + watchdog).
5. Kernel initializes the root address space set and the service
   table (§3), registers **only `init_service`**, and drops to U-mode.
6. `init_service` starts, in its own order (boot policy lives in
   user space): `supervisor_service`, `logger_service`,
   `console_service`, `shell_service`, then exits.
7. The console service owns input; the shell prints `axiom>` and
   becomes interactive.

Expected serial gate (roadmap §15):

```text
AxiomRT kernel booted
MMU status=enabled mode=sv39 scope=kernel
TASK_STARTED task=init_service
SERVICE started=supervisor_service
SERVICE started=logger_service
SERVICE started=console_service
SERVICE started=shell_service
axiom>
```

## 2. The user region (how services are real U-mode Rust)

The v0.9 demos ran self-contained assembly bodies because arbitrary
kernel Rust cannot run in U-mode: it references kernel `.text`/
`.rodata`, which user address spaces must not (and do not) map.

Services are therefore written as **constrained Rust** placed in a
dedicated linker region:

* All service functions carry `#[link_section = ".user.text"]`; all
  service constants are sectioned statics in `.user.rodata` (string
  literals are never used — they would land in kernel `.rodata`).
* The linker script gathers these into a page-aligned region
  `__user_text_start … __user_rodata_end` inside the kernel image.
* Every service address space maps that whole region contiguously at
  `USER_CODE_VA`: text pages U+R+X, rodata pages U+R (W^X preserved),
  plus one private U+R+W stack page per service.
* RISC-V medany code generation is pc-relative (`auipc`), so
  intra-region calls and static references work at the mapped address
  unchanged; **any reference that escapes the region is a page fault
  and is contained** — the isolation boundary itself enforces the
  constraint. Consequences the service author must respect: no core
  formatting, no panics, no compiler-emitted `memcpy` (manual byte
  loops only), everything it touches carries a `.user.*` section.

This is the "static built-in app table" stage of the application
model (roadmap §17 stage 1); a loader for external images replaces it
in the application phase.

## 3. Service table and sys_task_start

The kernel holds a static, boot-frozen service table (name, entry,
priority, capability grants — the mechanism). `init_service` holds a
task-control capability and starts each service by index (the
policy):

```text
sys_task_start (a7=8): a0 = service index
  -> builds the address space, registers the TCB, mints the service's
     capabilities from the table, marks it Ready
  errors: -2 no/insufficient control capability, -5 bad index,
          -7 already started / no free slot
```

Slots: MAX_TASKS = 8, MAX_USER_AS = 8 (init, supervisor, logger,
console, shell, faulty demo, critical demo, one spare).

## 4. New syscalls (numbers extend docs/04)

| # | name | args | gate |
|---|---|---|---|
| 8 | sys_task_start | a0=service index | control capability |
| 9 | sys_con_write | a0=buf VA, a1=len | console-write capability |
| 10 | sys_con_read | a0=buf VA, a1=max | console-read capability (console service only) |
| 11 | sys_info | a0=kind, a1=buf VA, a2=max | info capability |
| 12 | sys_task_kill | a0=task index | control capability |
| 13 | sys_task_restart | a0=task index | control capability |
| 14 | sys_shutdown | — | control capability |

* Console I/O is polled through the kernel's NS16550A driver (RX =
  LSR bit 0 + RBR; TX as before). The kernel offers the *mechanism*;
  the console **service** owns terminal policy (echo, line
  assembly) in user space. `sys_con_read` is non-blocking (returns 0
  when no byte), so the console service yields between polls and the
  system stays preemptible.
* `sys_info` fills the user buffer with a fixed `key=value` text
  snapshot (kinds: tasks, faults, ipc, caps, memory, uptime,
  events) — read-only introspection, no kernel state is mutable via
  this path.
* `sys_task_kill` marks a task Killed; `sys_task_restart` rebuilds
  its frame from the service table and marks it Ready. Both emit
  evidence lines.
* `sys_shutdown` prints a controlled-shutdown line and calls SBI SRST.
* Buffers must lie in the caller's stack page or (read side of
  sys_con_write only) the mapped user region; everything else is
  rejected with -5 before any copy (SUM-gated copies as in docs/17).

## 5. Capability layout (boot-minted, deny-by-default)

| service | capabilities |
|---|---|
| init_service | task control |
| supervisor_service | fault endpoint Recv+Control (as v0.8) |
| logger_service | event endpoint Recv |
| console_service | console read+write, shell-line endpoint Send |
| shell_service | console write, shell-line endpoint Recv, info, task control |
| faulty/critical demo tasks | none |

The shell holding task control is a documented policy decision of
this stage (it is the operator surface); per-application manifests
arrive with the app loader phase.

## 6. Verification

* tests/os_shell_qemu_test.sh: boots the `os_boot` build, pipes shell
  commands over stdin, asserts the §1 gate lines, the `axiom>`
  prompt, command output, and a controlled shutdown.
* Default build and all demo builds remain byte-for-byte governed by
  their existing tests (feature-gated, unchanged).
