# AxiomRT v1.0 — Limitations

This document states, explicitly and without hedging, what AxiomRT v1.0
does **not** do. It is part of the industrial evaluation kit and must be
read before drawing any conclusion from the demos.

**No certification claim. No production-readiness claim. No claim of
readiness for any vehicle, aircraft, or safety-critical deployment.**
AxiomRT v1.0 is an evaluation-stage prototype.

## Platform

* **Emulator only.** AxiomRT has run only on the QEMU `virt` machine
  (RISC-V 64) with the OpenSBI firmware bundled with QEMU. No physical
  board has ever run this kernel. Real-hardware bring-up is v1.1.
* **Single hart.** All scheduling, IPC, and fault logic assume one hart;
  there is no multicore support and no concurrency hardening.
* **4 KiB pages only** (Sv39); no megapages, no demand paging, no
  swapping, no copy-on-write.

## Scope enforced by design (v0.1 non-goals still hold)

No filesystem, no network stack, no GUI, no POSIX layer, no shell, no
package manager, no dynamic drivers, no user accounts, no shared-memory
IPC, no AI in the kernel. These keep the trusted computing base small.

## Functional boundaries at v1.0

* **Memory isolation** is MMU-enforced on QEMU **for the tested cases**
  (user read of kernel memory, user write of unmapped memory, user
  execute of a non-executable page). Untested access patterns are a
  documented gap.
* **On-target recovery** applies **Kill**. On-target **Restart**
  (re-create a task from its boot image) is not yet implemented; the
  safety-relevant property — the kernel survives a faulting task — holds.
* **Tasks are static**, created at boot from static descriptions; there
  is no dynamic task creation, and the demo tasks share position-
  independent code pages mapped per address space (no separate user-ELF
  loader).
* **IPC** is a single bounded message (64 bytes), one in-flight per
  endpoint, no queues; the on-target demo uses a small fixed set of
  endpoints.
* **Watchdog/deadline** windows are demo-tuned constants, not derived
  from WCET analysis.
* **Timer** is the QEMU virt time base via SBI; no calibrated wall clock.

## Verification boundaries

* Coq starter models prove the **model-level** theorems (memory
  isolation, capability access, scheduler priority) plus Sv39 encoding
  lemmas. The **refinement** from these models to the running Rust code
  is stated as explicit `TODO`s in each proof file — it is not yet
  discharged. No proof claim exceeds the verified relation.
* Tests are deterministic host tests plus QEMU serial-assertion tests;
  there is no fuzzing, coverage measurement, or independent review yet
  (v1.5+).

## What the kit does NOT constitute

* Not a safety case and not a certification package (those are
  v1.6/v1.7 and beyond).
* Not evidence of freedom from defects.
* Not a commitment to an API, ABI, or support level.
