# AxiomRT Capability Model

Document ID: created by AXIOM-CAP-001 (Phase 9)
Requirement reference: docs/03_KERNEL_OBJECTS.md §8, Project
Description §11, docs/02_KERNEL_BLUEPRINT.md §7.

(Naming note: the file number 06 collides with 06_FAULT_MODEL.md; both
names are fixed verbatim by the task pack.)

## 1. Principle

A capability is an explicit, unforgeable authority token:
**(object type, object id, rights)**. A task cannot access a protected
object because it knows an ID or an address — it must hold a valid
capability with sufficient rights in its capability table.

Unforgeability is structural: capabilities live only in kernel memory.
User code holds table *indexes*; capability bits never cross the
user/kernel boundary in either direction.

Protected objects (docs/03): threads, endpoints, address spaces,
physical frames, timers, scheduling contexts, fault channels.

## 2. Rights (`kernel/src/caps/rights.rs`)

Eight explicit rights, no implication between any pair (unit-tested):

```text
Read  Write  Execute  Send  Receive  Grant  Map  Control
```

* Checks are subset tests: an operation states its required rights and
  the held set must contain all of them.
* Rights can only **diminish** (`derive_diminished`); no operation adds
  rights to an existing capability — amplification does not exist.

## 3. Capability (`kernel/src/caps/capability.rs`)

* `ObjectRef` = type tag + kernel object ID. The type tag makes type
  confusion detectable at lookup: using an endpoint capability as a
  thread capability fails structurally (`WrongObjectType`).
* Minting is kernel-internal; in v0.1 all capabilities are minted at
  boot from static task descriptions. Grant-based transfer is a v0.2+
  decision (docs/03 §8).

## 4. Lookup Table (AXIOM-CAP-002)

Per-task `CapTable`, fixed capacity (no heap). The single enforcement
point: `lookup(index, expected_type, required_rights)` checks, in
order:

1. index in range and slot occupied → else `InvalidIndex`/`EmptySlot`
   (→ `ERR_INVALID_CAP`);
2. object type matches → else `WrongObjectType`
   (→ `ERR_WRONG_OBJECT_TYPE`);
3. held rights ⊇ required rights → else `InsufficientRights`
   (→ `ERR_INSUFFICIENT_RIGHTS`).

Only after all three does the caller receive the object reference. The
failure order is fixed and never reveals more than the first failing
check (docs/04, sys_send security rule).

## 5. IPC Integration (AXIOM-CAP-003)

* `sys_send` requires an Endpoint capability with **Send**;
* `sys_recv` requires an Endpoint capability with **Receive**;
* any lookup failure raises an **InvalidCapability** fault
  (docs/06_FAULT_MODEL.md) with a `CAP_DENIED`/`IPC_DENIED` event, and
  the endpoint is never touched.

IPC without a capability fails; IPC with a valid capability proceeds to
the rendezvous model of docs/08_IPC_MODEL.md. The rendezvous layer
itself contains no bypass: it is only reachable through the checked
syscall path.

## 6. Verification Targets

* **CAP-P1:** no protected object is invoked without a lookup success
  (→ proofs/coq/CapabilityAccess.v, Phase 12).
* **CAP-P2:** lookup succeeds only if held rights ⊇ required rights.
* **CAP-P3:** rights never grow along any derivation chain.
