# AxiomRT v1.7 Evidence Report — Minimal Network Service

Phase: AXIOM-NET (design: docs/34 and docs/35).
Tag: `v1.7-minimal-network-service` → `6b7e86b` (annotated and published).
Archived: 2026-09-03. Tool information: `tool_versions.txt`.
Status: verified release; all v1.7 gates pass.

## 1. What v1.7 demonstrates

v1.7 implements a synthetic user-space network service. The isolated
`net_driver_service` and `net_service` tasks run in U-mode; the kernel keeps
only generic task, capability, bounded-IPC, device, event, and containment
mechanisms. The driver exposes a deterministic 64-byte test-packet model:
one send increments TX once and synthetic loopback increments RX once.

The shell exposes network status, statistics, `send-test`, `rx-count`,
deliberate driver fault, and restart operations. Structured `NET_DRIVER`,
`NET_SERVICE`, `NET_TX`, `NET_RX`, and `NET_DENIED` events are parsed by
`axiomctl` and displayed by Studio with state, counters, mode, fault, and
restart information.

Real-machine QEMU verification confirms that a deliberate network-driver
U-mode fault is contained to `net_driver_service`; the shell and
`net_service` remain alive, the driver-down result is bounded, and a
manager-requested restart restores the driver with counters reset.
Application, filesystem, storage, and unrelated driver paths remain usable
after containment and restart.

## 2. Final verification

The authoritative real-machine logs are
`network_service_qemu_test.log` and `verify_all.log`.

Runtime verification:

* OS shell QEMU test: PASS;
* driver framework QEMU test: PASS;
* network service QEMU test: PASS;
* full sweep: 16/16 QEMU tests;
* final result: `VERIFY ALL: PASS`;
* controlled shutdown succeeds and the network test observes no kernel panic.

Host verification:

* kernel host suites: PASS (116 library, 4 binary, 30 integration tests);
* axiomctl: 17/17 PASS;
* supervisor: 4/4 PASS;
* Studio: 6/6 PASS;
* `cargo fmt --check`: PASS;
* host-target workspace Clippy with `-D warnings`: PASS;
* bare-kernel Clippy with `-D warnings`: PASS.

Coq verification:

* `MemoryIsolation.v`, `CapabilityAccess.v`, and
  `SchedulerPriority.v` compile successfully;
* current Stdlib-prefix deprecation warnings remain;
* those warnings are not proof failures.

Exact QEMU and Coq version strings were not captured in the authoritative
logs and are therefore not invented in `tool_versions.txt`.

## 3. Release-blocker found by real-machine verification

ROOT CAUSE: v1.7 increased `dispatch::MAX_TASKS` from 14 to 16 while
`paging_hw::MAX_USER_AS` remained fixed at 14.

EFFECT: `net_driver_service` uses task/address-space index 14.
Address-space construction rejected that index, so `os_boot` stopped before
`net_driver_service` and `net_service` startup and before the `axiom>`
prompt. This was address-space capacity drift, not a `driver_manager`
deadlock.

FIX: commit `6b7e86b`,
`AXIOM-NET-FIX-001: repair v1.7 service startup sequencing`, makes
`MAX_USER_AS` derive from `dispatch::MAX_TASKS`. The two related
capacities can no longer drift independently.

POST-FIX RESULT:

```text
16/16 QEMU tests
VERIFY ALL: PASS
```

The annotated tag identifies the verified runtime code at `6b7e86b`. This
successful evidence refresh is archived on main immediately after that
published tag; the tag was not rewritten.

## 4. Explicit limitations

This is synthetic networking only. v1.7 has no TCP/IP, sockets, Ethernet,
ARP, UDP, TCP, DNS, routing, or TLS. It makes no internet-support claim, no
production-network claim, no real-hardware-network validation claim, and no
network-security claim. It does not implement virtio-net, device DMA, or
real network interrupts.

There is no DO-178C compliance or certification claim. AxiomRT remains an
emulator-oriented research/high-assurance prototype; this archive is not
DO-178C, ISO 26262, IEC 61508, or equivalent certification evidence.

## 5. Next phase

The next planned phase is `v1.8-robustness-fuzzing`. Fuzzing evidence must
remain distinct from formal proof and must not be presented as production,
real-hardware, or certification evidence.
