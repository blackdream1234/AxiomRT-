# 33 — Storage-Backed Filesystem Transition

Document ID: created by AXIOM-LOAD-002 (Phase v1.6).
Requirement reference: `AxiomRT v1.6.md`, docs/28, docs/29, docs/32.

## 1. Where v1.3 left the filesystem

`fs_service` (docs/28) serves a read-only tree from `.user.rodata`
statics: fixed paths, one bounded `LS`/`CAT` request in, one bounded
reply out, malformed input always answered with `ERR`. All path policy
is in the service; the kernel is transport only.

## 2. Where v1.4 left storage

`storage_service` (docs/29) serves a static 8×48-byte read-only block
image over `INFO` / `READ block=<n>` / `READ_RANGE`. One client-visible
chain already crosses both services: `cat /storage/version` = shell →
fs → storage → fs → shell (nested bounded IPC on the fs service's own
storage capability).

## 3. The v1.6 storage-backed app image path

v1.6 adds `/bin` to the fs tree, holding restricted app image records
(docs/32):

```text
/bin/hello.app                 <- storage block 4
/bin/counter.app               <- storage block 5
/bin/fault_demo.app            <- storage block 6
/bin/invalid_bad_magic.app     <- fs-static (test fixture)
/bin/invalid_bad_cap.app       <- fs-static (test fixture)
/bin/invalid_bad_checksum.app  <- fs-static (test fixture)
```

For the three valid apps, `CAT /bin/<name>.app` makes fs_service fetch
the record from storage_service (`READ block=4|5|6`), strip the
`OK data=` frame, and forward the bare record — the same nested-IPC
pattern as `/storage/version`, now carrying loader-consumed data. The
three invalid records are deliberately corrupt test fixtures and stay
fs-static (they exist to prove rejection, not storage transport).
Storage blocks 4–6 were `reserved` filler in v1.4; giving them content
extends the image without changing the storage protocol, block size,
block count, or error behavior.

## 4. Why the filesystem remains user-space

Path parsing, directory layout, and "which bytes answer which name"
are pure policy. A path-parsing bug in v1.6 is a contained fs_service
fault, not a kernel compromise; the kernel still never sees a path.
This also keeps the future writable filesystem (§9) a user-space
project: journaling and consistency policy must not enter the kernel.

## 5. Why storage remains user-space

Block access policy (which block, caching, retries, and eventually the
virtio driver behind it — docs/31 §15) is driver/service policy. The
kernel's storage contribution stays what it was in v1.4: endpoint
transport and capability rights bits. When `block_driver_service`
becomes a real virtio driver, `storage_service` swaps its static image
for driver IPC and this document's paths do not change.

## 6. How fs_service asks storage_service for image data

fs_service holds an endpoint capability for the storage channel
(slot 1: Send | Recv | storage_read — minted at boot, docs/29 §5). On
`CAT /bin/hello.app` it sends `READ block=4` (≤ 64 bytes), receives
one bounded reply, verifies the `OK data=` prefix, and forwards the
payload. A storage error (`ERR …`) or an IPC failure is answered to
the client as `ERR not_found` — fs never retries, never blocks
unboundedly beyond the rendezvous, and never invents content.

## 7. How app_loader asks fs_service for app metadata

`app_loader_service` gains (v1.6) an fs endpoint capability with read
authority. On `APP_LOAD <name>` it sends `CAT /bin/<name>.app`,
receives the record in one bounded reply, and runs the docs/32 §6
validation chain. The loader never talks to storage directly — layering
is loader → fs → storage, each hop capability-checked, each message
bounded. The kernel routes IPC and checks rights; it reads none of it.

## 8. Limitations

* The block image and fs tree are still build-time static; "storage-
  backed" means the bytes travel the real service chain, not that they
  are persistent or external.
* One record = one block = one IPC reply; images larger than one
  bounded message are future work (multi-block fetch protocol).
* No cache: every load re-fetches; acceptable at shell interaction
  rates.
* Failure mapping is coarse (`ERR not_found` for any storage-side
  failure) — deliberate, to keep client-visible vocabulary small.

## 9. Future writable filesystem

Requires: write-capable storage protocol (`WRITE block=<n>` with an
explicit write right), a journaling/consistency policy in fs_service
(user space), quota/ownership policy, and a documented crash-
consistency claim backed by tests — none of which exists in v1.6, and
no persistence is claimed (docs/20 tier rules).

## 10. Future block-backed persistence

The chain assembled across v1.4–v1.6 (client → fs → storage →
[driver]) is the persistence path: storage_service swaps its static
image for `block_driver_service` IPC (docs/31 §15), the driver drives
a real virtio-blk device (docs/30 §5), and QEMU gets a `-drive`
backing file. Then `/bin` records (and later real image bytes) come
from an actual disk image, with the same interfaces this phase fixed.
