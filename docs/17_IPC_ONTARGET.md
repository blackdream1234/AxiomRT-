# AxiomRT On-Target IPC

Document ID: created by AXIOM-IPCRT-001 (v0.6, Stage 5)
Requirement reference: docs/08_IPC_MODEL.md (host model),
docs/13_DISPATCH.md, docs/12_MMU_SV39.md, Full Completion Mode §15.

## 1. Goal and Boundary

v0.6 runs synchronous, bounded, copy-based IPC between two U-mode tasks
on target, realizing the host IPC model (docs/08) with real
cross-address-space message copying and blocking. Scope: user buffer
validation, `sys_send`/`sys_recv`, sender/receiver blocking, bounded
kernel-buffered copy, peer handling. Capability enforcement on the IPC
path is the **next** stage (v0.7, AXIOM-CAPRT); v0.6 uses a single demo
endpoint without a capability check. No shared memory. Demo behind the
`demo_ipc` cargo feature; default build unchanged.

## 2. Message Buffer and Bound

One bounded message at a time (`IPC_MSG_MAX = 128` bytes (64 before v1.6; raised for the /bin listing, docs/33 §3)) is staged in a
kernel buffer (`KMSG`). There is no queue and no shared memory: the
payload is copied sender→kernel and kernel→receiver, so the two tasks
never alias memory (docs/08 §1).

## 3. Endpoint State

A single demo endpoint (`Ep`):

```text
Idle
SenderWaiting   { tid, len }        sender parked, message staged in KMSG
ReceiverWaiting { tid, dst, cap }   receiver parked, awaiting a sender
```

## 4. User Buffer Validation (AXIOM-IPCRT-002/003)

A user IPC buffer `[va, va+len)` must lie entirely inside the task's
mapped user data window (`USER_DATA_VA .. USER_DATA_END`, the user stack
page) and `len ≤ IPC_MSG_MAX`. Invalid buffers are rejected **before any
copy** with `ERR_INVALID_ARG` / `ERR_MSG_TOO_LARGE` and an `IPC_DENIED`
event (docs/06, IPCViolation). The kernel copies user memory only inside
the SUM-gated `copy_from_user`/`copy_to_user` routines (sstatus.SUM is
set only for the duration of a validated copy, then cleared).

## 5. Rendezvous (AXIOM-IPCRT-004..009)

`sys_send(a1=buf, a2=len)`:

* endpoint `Idle` → copy sender buffer into KMSG, park sender
  (`SenderWaiting`, state Blocked), switch away — **send blocks if no
  receiver**;
* endpoint `ReceiverWaiting` → copy sender buffer into KMSG, stage a
  deferred delivery on the receiver, wake it (Ready), sender continues.

`sys_recv(a1=buf, a2=cap)`:

* endpoint `Idle` → validate buffer, park receiver (`ReceiverWaiting`,
  state Blocked), switch away — **receive blocks if no sender**;
* endpoint `SenderWaiting` → copy KMSG into the receiver's buffer
  (receiver satp active), return length, wake the sender.

**Deferred delivery (AXIOM-IPCRT-006):** when a send finds a waiting
receiver, the receiver is not currently running (its satp is inactive),
so the kernel→receiver copy is deferred. Every resume path
(`resume_task`) completes a pending delivery once the target address
space is active, then reports `IPC delivered bytes=N`.

Peer death (AXIOM-IPCRT-009): a task killed while its peer is parked
leaves the endpoint recoverable; a bounded second sender/receiver is
rejected (`busy`) — v0.6 has no queue.

## 6. Expected QEMU Output (demo_ipc, receiver-first)

```text
MMU status=enabled mode=sv39 scope=kernel
TASK_STARTED task=receiver
TASK_STARTED task=sender
IPC recv task=receiver
IPC endpoint=log op=recv state=blocked
SCHED selected=sender
IPC send task=sender
IPC endpoint=log op=send
SYSCALL name=sys_exit task=sender
TASK_EXITED task=sender
SCHED selected=receiver
IPC delivered bytes=4
SYSCALL name=sys_exit task=receiver
TASK_EXITED task=receiver
SCHED idle=all_tasks_done
```

## 7. Test (AXIOM-IPCRT-010)

`tests/ipc_rendezvous_qemu_test.sh` builds with `--features demo_ipc`,
boots, and asserts the receiver blocks, the sender sends, the message is
delivered (`IPC delivered bytes=4`), both tasks exit, and no `PANIC`
appears.
