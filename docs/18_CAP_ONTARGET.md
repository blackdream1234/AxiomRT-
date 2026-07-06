# AxiomRT On-Target Capability Enforcement

Document ID: created by AXIOM-CAPRT-001 (v0.7, Stage 6)
Requirement reference: docs/06_CAPABILITY_MODEL.md (host model),
docs/17_IPC_ONTARGET.md, Full Completion Mode §16.

## 1. Goal and Boundary

v0.7 makes every on-target IPC operation capability-controlled: a task
may send or receive on the endpoint only if it holds a capability with
the matching right. This realizes the host capability model (docs/06)
on the running kernel. Scope: boot-time capability minting, per-task
capability tables, Send/Receive enforcement on `sys_send`/`sys_recv`,
and `CAP_DENIED` events. The demo is the `demo_cap` cargo feature; the
IPC demo (`demo_ipc`) is updated to carry capabilities so it remains a
valid, now capability-backed, exchange.

## 2. Capability Representation (AXIOM-CAPRT-001/002)

Each task control block carries a small fixed capability table
(`CAPS_PER_TASK` slots). A capability is
`{ object type, object id, rights }` — the on-target form of the host
`Capability` (docs/06 §3). Capabilities are minted at boot
(`set_task_cap`) from the static task description; user code holds only
a **table index**, never capability bits (unforgeability is structural,
docs/06 §1).

Rights bits match the host model: `Send = 1<<3`, `Receive = 1<<4`
(docs/06 §2).

## 3. Enforcement (AXIOM-CAPRT-003/004/005/006/007)

`sys_send(a0 = cap_index, a1 = buf, a2 = len)` and
`sys_recv(a0 = cap_index, a1 = buf, a2 = cap)` resolve `cap_index` in
the **caller's** capability table before touching the endpoint. The
lookup, in fixed order (docs/06 §4):

1. index in range and slot occupied — else fail;
2. object type is Endpoint and object id matches the endpoint — else
   fail;
3. rights include the required right (Send for send, Receive for
   receive) — else fail.

On any failure the syscall returns the specific error
(`ERR_INVALID_CAP` / `ERR_WRONG_OBJECT_TYPE` /
`ERR_INSUFFICIENT_RIGHTS`), emits `CAP_DENIED task=<name>
reason=no_valid_capability`, reports `IPC state=unchanged`, and **the
endpoint is never touched** (deny-by-default; the object is unchanged).
Only after a successful lookup does the rendezvous of docs/17 run.

## 4. Expected QEMU Output (demo_cap)

`receiver` (Receive cap) blocks; `faulty_task` (no cap) is denied;
`good_sender` (Send cap) delivers:

```text
TASK_STARTED task=receiver
TASK_STARTED task=faulty_task
TASK_STARTED task=good_sender
IPC recv task=receiver
IPC endpoint=log op=recv state=blocked
SCHED selected=faulty_task
CAP_DENIED task=faulty_task reason=no_valid_capability
IPC state=unchanged
TASK_EXITED task=faulty_task
SCHED selected=good_sender
IPC send task=good_sender
IPC delivered bytes=4
```

## 5. Test (AXIOM-CAPRT-008)

`tests/capability_qemu_test.sh` builds with `--features demo_cap`,
boots, and asserts: the capability-less send is denied
(`CAP_DENIED ... reason=no_valid_capability`, `IPC state=unchanged`),
the capable send then delivers (`IPC delivered bytes=4`), and no
`PANIC` appears. The host capability unit tests
(`coqc proofs/coq/CapabilityAccess.v`, `capability_tests.rs`) continue
to cover the model-level properties.
