# AxiomRT v1.0 — Security Concept

Evaluation-stage security concept. Every claim references a test or
proof; no claim exceeds the evidence.

## 1. Trusted Computing Base (TCB)

The TCB is the AxiomRT microkernel (the only code at supervisor
privilege) plus the boot assembly. OpenSBI / machine mode is a platform
assumption, not part of the TCB claim (ASSUMPTIONS_OF_USE.md). The
supervisor task is trusted for **policy only** — it runs at user
privilege and cannot bypass isolation or capability checks.

## 2. Attack Surface

The only paths from a user task into the kernel are:

* **syscalls** (`ecall` from U-mode) — every argument validated before
  use (docs/04_SYSCALL_MODEL.md);
* **exceptions** (page fault, illegal instruction) — contained
  (docs/06, docs/10, docs/12);
* **timer interrupts** — kernel-internal, not user-controlled.

There is no filesystem, network, shared memory, or dynamic loader to
attack (LIMITATIONS.md).

## 3. Security Mechanisms and Evidence

| Property | Mechanism | Evidence |
|---|---|---|
| No unauthorized object access | capability lookup before every protected operation; deny-by-default | capability_qemu_test, proofs/coq/CapabilityAccess.v, capability_tests.rs |
| No privilege escalation | U/S privilege split; kernel pages U=0; privileged ops trap | memory_isolation_qemu_test, full demo (illegal instruction contained) |
| No cross-task memory access | per-task Sv39 address spaces, single-frame ownership | memory_isolation_qemu_test, proofs/coq/MemoryIsolation.v |
| No forged authority | capabilities live only in kernel memory; user holds indexes | docs/18, capability model |
| No unbounded input effect | bounded copy-based IPC; syscall arg validation | ipc_rendezvous_qemu_test, ipc_tests.rs |
| Faults are attributable | structured fault/monitor events to supervisor + logger | supervisor_qemu_test |

## 4. Deny-by-Default

Every protected operation fails closed:

* an IPC send/recv without a valid capability is denied with the
  endpoint unchanged (`CAP_DENIED`, `IPC state=unchanged`);
* an unknown syscall returns `ERR_INVALID_SYSCALL`;
* an invalid user buffer is rejected before any copy.

Verified on target (capability_qemu_test, full_fault_containment_demo).

## 5. Information-Flow Limitations

* v1.0 does not analyze covert timing channels.
* The supervisor/logger receive fault descriptors, not full task memory;
  no user memory is exported to another task except through explicit,
  capability-checked, bounded IPC.
* Side-channel resistance is out of scope for v1.0.

## 6. Residual Security Risk (evaluation scope)

* No fuzzing / no adversarial test campaign yet (v1.5).
* Refinement of the capability model to code not yet discharged.
* Single-hart; no concurrency attack surface analyzed.
