# AxiomRT IPC Model

Document ID: created by AXIOM-IPC-001 (Phase 8)
Requirement reference: docs/02_KERNEL_BLUEPRINT.md §9, Project
Description §14, docs/03_KERNEL_OBJECTS.md §6–§7.

(Naming note: the file number 08 collides with 08_PHASE_0_GATE.md; both
names are fixed verbatim by the task pack.)

## 1. Principles

IPC v0.1 is:

* **synchronous** — a rendezvous between exactly one sender and one
  receiver; whoever arrives first blocks;
* **bounded** — fixed maximum message size, fixed one in-flight message
  per endpoint, no queues;
* **copy-based** — payload bytes are copied at send and copied again at
  delivery; sender and receiver never alias memory;
* **capability-controlled** — Send/Receive rights checked at the
  syscall layer (Phase 9); the model below never bypasses them.

Shared memory IPC is forbidden in v0.1 (docs/01 §2): it increases proof
complexity and would break the copy-only information flow.

## 2. Message (`kernel/src/ipc/message.rs`)

* `MSG_MAX_BYTES = 64`. Oversized payloads are rejected **before any
  copy** (`MessageError::TooLarge` → `ERR_MSG_TOO_LARGE`).
* A `Message` is a copy: mutating the source buffer after construction
  cannot affect it (unit-tested).
* The header (sender `ThreadId`, length) is kernel-written and
  unforgeable by user code (docs/04, sys_recv security rule).

## 3. Endpoint (`kernel/src/ipc/endpoint.rs`)

States (docs/03 §6):

```text
Idle ── send ──> SenderWaiting ── recv ──> Idle   (message delivered)
Idle ── recv ──> ReceiverWaiting ── send ──> Idle (message delivered)
```

* Exactly one party can wait per endpoint side; a second concurrent
  sender (or receiver) is an explicit `Busy` error, not a queue —
  boundedness is structural.
* `Transferring` is atomic at model level: a transfer completes inside
  one kernel operation; no intermediate state is observable.
* Exactly one in-flight message can exist (`pending`), held while a
  sender waits.

## 4. Rendezvous Semantics (AXIOM-IPC-002)

Without a scheduler integration, "blocking" is represented as an
explicit outcome the kernel acts on (the thread state machine moves the
party to Blocked):

* `send(sender, msg)`:
  * endpoint Idle → sender parks (`SenderWaiting`), outcome
    **Blocked** — send blocks if no receiver;
  * `ReceiverWaiting(r)` → message delivered to `r`, endpoint Idle,
    outcome **Delivered{to: r}** — both parties continue;
  * `SenderWaiting` → outcome **Busy** (bounded: no sender queue in
    v0.1).
* `recv(receiver)`:
  * endpoint Idle → receiver parks (`ReceiverWaiting`), outcome
    **Blocked** — receive blocks if no sender;
  * `SenderWaiting(s)` → outcome **Received{msg, unblock: s}** — the
    waiting sender is released, endpoint Idle;
  * `ReceiverWaiting` → outcome **Busy**.
* `cancel(tid)` (kill path, docs/03 §6 failure behavior): if `tid` is
  the parked party, the rendezvous is cancelled, the endpoint returns
  to Idle, and the caller learns whether a peer must be unblocked with
  `ERR_PEER_KILLED`.

Determinism: the outcome is a pure function of (endpoint state,
operation); there is no timing dependence and no hidden queue order.

## 5. Explicitly Absent in v0.1

* shared memory transfer of any kind
* asynchronous / buffered sends, sender or receiver queues
* broadcast or multicast endpoints
* capability transfer through messages (Grant is a v0.2+ decision,
  docs/03 §8)
* zero-copy optimizations (proof simplicity wins over throughput)
