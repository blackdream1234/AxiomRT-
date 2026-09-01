# 34 — Minimal Network Service

Document ID: created by AXIOM-NET-001 (Phase v1.7).
Requirement reference: `AxiomRTv1.7.md`, docs/25, docs/31.

## 1. Why networking is user-space

Network drivers and protocol code process externally influenced data and
therefore form a large, failure-prone trust boundary. AxiomRT keeps them out
of S-mode. Each network component is an ordinary U-mode service with its own
Sv39 address space and an explicit capability table. A fault stops only that
service; the kernel, shell, storage, filesystem, applications, and unrelated
drivers remain available.

The kernel supplies mechanisms only: task isolation and scheduling, bounded
copy-based IPC, capability lookup, device identity, modeled DMA/IRQ grants,
and fault containment. It does not decide packet policy or interpret network
requests.

## 2. Service split

`net_driver_service` owns the low-level synthetic device endpoint. It holds
the `net0` device capability, maintains bounded TX/RX counters, accepts the
driver protocol, and contains no Ethernet/IP/TCP policy.

`net_service` is the user-facing network policy boundary. It accepts bounded
requests from the shell, checks its service state, translates them to the
driver protocol, and returns bounded replies. It has no device capability and
cannot access MMIO or DMA.

`driver_manager` owns network-driver lifecycle policy. It observes the
deliberate fault through the existing synthetic-IRQ liveness mechanism and
requests restart through its task-control capability. The shell never talks
to `net_driver_service` or `net0` directly.

## 3. v1.7 implementation

v1.7 implements:

* isolated U-mode `net_driver_service` and `net_service` tasks;
* a synthetic `net0` device identity;
* fixed-size IPC messages with no heap allocation;
* a deterministic test packet represented only by its bounded byte count;
* monotonically increasing in-service TX/RX counters;
* status, statistics, send-test, receive-count, deliberate-fault, and restart
  operations;
* explicit network rights and deny-by-default grants;
* structured network events for host tools and evidence;
* deterministic QEMU verification with no external network dependency.

The synthetic TX operation increments TX once and models loopback reception
by incrementing RX once. No arbitrary packet bytes enter the kernel, and no
external host or guest interface is opened.

## 4. Explicit non-goals

v1.7 does not implement a production network stack.
v1.7 does not implement TCP/IP.
v1.7 does not implement sockets, Ethernet parsing, ARP, IP, UDP, TCP, DNS,
TLS, routing, or firewall policy.
v1.7 does not provide internet connectivity guarantees.
v1.7 does not claim network security, production networking, real-hardware
networking, or certification.

## 5. Packet model

The only packet is `test_packet`, with a fixed documented length of 64 bytes.
Its contents are not supplied by the shell and are not parsed. `DRV_TX_TEST`
performs one bounded counter transition:

```text
tx := tx + 1
rx := rx + 1
```

The second transition is a synthetic loopback observation. Counters are
service-local `u64` values and restart at zero when the driver task is
restarted. No queue can grow: the IPC layer permits one bounded rendezvous per
endpoint and the model holds no packet backlog.

## 6. Bounded IPC protocol

Shell to `net_service` over the network endpoint:

```text
NET_STATUS
NET_STATS
NET_SEND_TEST
NET_RX_COUNT
NET_FAULT
NET_RESTART
```

`net_service` to `net_driver_service` over the driver endpoint:

```text
DRV_STATUS
DRV_TX_TEST
DRV_RX_COUNT
DRV_FAULT
DRV_RESTART
```

Lifecycle requests are coordinated with `driver_manager`; ordinary status,
TX, and RX requests go directly from `net_service` to the driver. Messages
are limited by the global 128-byte IPC bound and each service uses fixed
64/128-byte stack buffers. Exact commands are accepted; empty, truncated,
extended, or unknown commands return `ERR malformed` or `ERR unsupported`.
Oversized messages are rejected by the kernel IPC mechanism before delivery.

Representative replies are:

```text
OK net state=up driver=running tx=<n> rx=<n> mode=synthetic
OK tx=<n> rx=<n>
OK sent test_packet bytes=64
OK rx_count=<n>
ERR denied
ERR malformed
ERR driver_down
ERR too_large
ERR unsupported
```

## 7. Capability model

Network authority is represented by declarative endpoint rights:

```text
net_status   query service/driver status
net_tx       request the bounded test transmission
net_rx       query the receive counter
net_control  request deliberate fault or restart
```

Grants are deny-by-default:

* shell: network endpoint with status/TX/RX/control; no `net0` device cap;
* `net_service`: client and driver IPC authority plus lifecycle coordination;
  no device cap;
* `net_driver_service`: driver IPC authority and the `net0` device/IRQ grant;
* `driver_manager`: network-driver lifecycle/IRQ control only;
* applications, including `hello`, `counter`, and `fault_demo`: no network
  endpoint and no network device authority.

The explicit per-task capability capacity increase is verified by the QEMU
test; no pre-v1.7 shell capability is removed.

## 8. Fault containment and restart

`net fault` requests the documented deliberate `net_driver_service` page
fault. The invalid access is made in U-mode. The existing trap/fault path
marks only that task Faulted, drops pending device events, informs the
supervisor/logger, and keeps the kernel and shell alive. `driver_manager`
uses a bounded synthetic-IRQ probe to observe the failed receiver and records
the network driver as faulted.

While faulted, `net_service` returns `ERR driver_down` without sending to an
endpoint that has no receiver. `net restart` asks `driver_manager` to re-arm
the original task frame and grants; the manager injects a fresh bounded
attention event and the driver returns to its receive loop.

## 9. Security limitations

The model does not process untrusted network frames and therefore cannot
demonstrate protocol-parser security. It provides isolation and least
privilege for the modeled path only. It has no authentication,
confidentiality, integrity, replay defense, firewall, rate limiting, IOMMU,
or hostile-device model. A future real driver and stack require separate
threat analysis and fuzzing.

## 10. Safety limitations

Containment preserves unrelated services, not network availability. A
faulted driver loses counters and cannot serve traffic until restart. The
synthetic loopback path does not test DMA corruption, interrupt storms,
packet loss, timing, reordering, congestion, or hardware reset. No safety or
certification claim follows from the demonstration.

## 11. Future TCP/IP path

A future U-mode stack may sit above `net_service` (or replace its synthetic
policy) and use a separate bounded packet IPC contract. Ethernet, ARP, IP,
UDP, TCP, socket-like APIs, routing, and application policy must remain in
user space. The kernel interface should remain unchanged unless a new generic
mechanism is independently justified and capability-gated.

## 12. Future virtio-net path

`net_driver_service` can replace the synthetic counter transition with
virtio-net feature negotiation and bounded virtqueue operations once the
requirements in docs/35 are implemented. Clients keep the same service
boundary; only the driver implementation and declared mode change.

## 13. Future real-hardware path

A board support package must provide device discovery, exact MMIO/IRQ data,
DMA-safe memory and cache maintenance, IOMMU policy where available, and a
verified reset sequence. Hardware-backed evidence must be separate from the
v1.7 synthetic evidence.

## 14. Kernel boundary

The kernel contains task, IPC, capability, device, DMA/IRQ, and fault
mechanisms. Network service policy, request interpretation, packet counters,
test-packet behavior, fault/restart decisions, and shell commands remain in
U-mode. The kernel contains no packet parser, network stack, socket API,
addressing, routing, DNS, firewall, or application network policy.
