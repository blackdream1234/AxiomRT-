# Fault Containment Demo

Requirement reference: docs/DEMO_SCENARIO.md (read that first — it is
self-contained and explains the scenario, the current v0.1 subset,
and the expected output line by line).

## Run it

From the repository root:

```sh
./scripts/run_qemu.sh
```

Exit QEMU with `Ctrl-A` then `x`.

What you will see: the kernel boots through OpenSBI, drops a demo
task to user privilege, answers its syscalls through the trap path,
denies its capability-less IPC attempt (`CAP_DENIED`), then contains
its privileged-instruction fault (`CONTAIN ... kernel=alive`) and
keeps running (`kernel=survived`). The exact expected serial output
is documented in docs/DEMO_SCENARIO.md §3.

## Automated check

```sh
./tests/boot_smoke_test.sh
```

## Scope note

v0.1 runs one user task on target; the four-task scenario
(critical / supervisor / logger / faulty) is specified in
docs/DEMO_SCENARIO.md §1, with its policy chain (fault event →
supervisor decision → recovery) running today as deterministic host
tests. Limitations: docs/INDUSTRIAL_EVALUATION_KIT.md §9.
