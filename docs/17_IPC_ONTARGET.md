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

A message is bounded at `IPC_MSG_MAX = 128` bytes (64 before v1.6;
raised for the /bin listing, docs/33 §3). There is no queue and no
shared memory: the payload is copied sender→kernel and kernel→receiver,
so the two tasks never alias memory (docs/08 §1).

**Payload ownership (AXIOM-FOUND-001).** A message parked by a blocked
sender belongs to *its endpoint*, matching the approved model in
docs/08 §3 ("exactly one in-flight message can exist, held while a
sender waits"). The dispatcher keeps one bounded slot per endpoint,
`EP_MSG[NUM_ENDPOINTS][IPC_MSG_MAX]`; slot *i* holds the payload of
endpoint *i*'s parked sender and is meaningful only while that
endpoint is `SenderWaiting`.

Earlier revisions staged every message in a single kernel buffer
(`KMSG`). That was never sound: a second send on the *same* endpoint
overwrote the parked payload before being rejected as busy, which was
reachable even with one endpoint. Growing to `NUM_ENDPOINTS = 12`
added a second failure — a send on any other endpoint overwrote an
unrelated parked message — but did not create the defect. `KMSG` has
been removed; the copy helpers now take an explicit kernel-side buffer.

## 3. Endpoint State

Endpoint state (`Ep`), one instance per endpoint (`NUM_ENDPOINTS = 12`):

```text
Idle
SenderWaiting   { tid, len }        sender parked; bytes in EP_MSG[ep][0..len]
ReceiverWaiting { tid, dst, cap }   receiver parked, awaiting a sender
```

The payload slot is filled **before** `SenderWaiting` is published and
cleared **before** the endpoint returns to `Idle`, so no observable
state has `SenderWaiting` without its bytes, and no released slot
retains a previous message.

## 4. User Buffer Validation (AXIOM-IPCRT-002/003)

A user IPC buffer `[va, va+len)` must lie entirely inside the task's
mapped user data window (`USER_DATA_VA .. USER_DATA_END`, the user stack
page) and `len ≤ IPC_MSG_MAX`. Invalid buffers are rejected **before any
copy** with `ERR_INVALID_ARG` / `ERR_MSG_TOO_LARGE` and an `IPC_DENIED`
event (docs/06, IPCViolation). The kernel copies user memory only inside
the SUM-gated `copy_from_user`/`copy_to_user` routines (sstatus.SUM is
set only for the duration of a validated copy, then cleared).

## 5. Rendezvous (AXIOM-IPCRT-004..009)

Validation precedence is unchanged and every check completes **before
any byte is copied**: capability with the required right, then
`len > IPC_MSG_MAX` → `ERR_MSG_TOO_LARGE`, then buffer range →
`ERR_INVALID_ARG`, then the endpoint state decides.

`sys_send(a1=buf, a2=len)`:

* endpoint `Idle` (**sender-first**) → copy sender buffer into
  `EP_MSG[ep]`, then park the sender (`SenderWaiting`, state Blocked)
  and switch away — **send blocks if no receiver**;
* endpoint `ReceiverWaiting` (**receiver-first**) → copy sender buffer
  directly into the waiting receiver's deferred-delivery buffer, wake
  it (Ready), sender continues. `EP_MSG` is not used on this path;
* endpoint `SenderWaiting` → `ERR_INVALID_ARG` (busy). **No copy is
  performed**, so the parked payload, the endpoint state and every
  unrelated task are unaffected. As required by docs/04 the caller's
  `a0` is set and an `IPC_DENIED` event is emitted; those are the only
  observable changes.

`sys_recv(a1=buf, a2=cap)`:

* endpoint `Idle` → validate buffer, park receiver (`ReceiverWaiting`,
  state Blocked), switch away — **receive blocks if no sender**;
* endpoint `SenderWaiting` with `len > cap` → `ERR_INVALID_ARG`; the
  sender **stays parked** and its payload is untouched, so an
  adequately sized retry still returns the original bytes;
* endpoint `SenderWaiting` → copy `EP_MSG[ep][0..len]` into the
  receiver's buffer (receiver satp active), clear the slot, return the
  length, wake the sender.

**User-copy fault boundary.** The SUM-gated copies run in S-mode, so
`TrapFrame::is_from_user()` is false for a fault taken inside them: the
containment path in the trap handler does not apply and the kernel
halts with `PANIC … reason=kernel_page_fault`. This design guarantees
only that **validation rejects before any copy begins**; it provides no
recovery from an unexpected fault during a copy, and none is claimed. A
recoverable-copy mechanism would need a trap-path fixup and is proposed
separately, not implemented here.

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
