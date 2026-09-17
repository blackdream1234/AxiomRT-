# AxiomRT v1.7 Prompt — AXIOM-NET Minimal User-Space Network Service

You are working on AxiomRT.

Repository:

```text
https://github.com/blackdream1234/AxiomRT-
```

Current local verified state:

AxiomRT has reached `v1.6-storage-backed-loader`.

Current capabilities:

* QEMU RISC-V 64 / OpenSBI boot.
* Sv39 MMU enabled.
* Isolated U-mode services.
* Interactive `axiom>` shell.
* app_loader_service.
* Static and restricted storage-backed user applications.
* Read-only fs_service.
* storage_service.
* driver_manager.
* block_driver_service skeleton.
* device object/capability model.
* capability-gated MMIO.
* modeled DMA bounce-page grant.
* synthetic IRQ delivery.
* watchdog containment.
* supervisor/logger recovery.
* `axiomctl`.
* AxiomRT Studio.
* install script.
* evidence archives.
* `./scripts/verify_all.sh -> VERIFY ALL: PASS`.
* 15/15 QEMU tests.
* zero warnings.
* clippy `-D warnings` clean.
* 3 Coq model files compile.

Known v1.6 limitations:

* Not full ELF.
* No writable storage.
* No persistence.
* No real hardware.
* No certification claim.
* v1.6 restricted app records reference statically-present `.user` code.
* IPC message bound is now 128 bytes; host and target constants must remain synchronized.
* Dense/mixed `.user` switches can cause LLVM rodata-table escape into kernel `.rodata`; use the established branch-per-arm helper rule.

Known v1.5 limitations still active:

* No full virtio-blk.
* DMA is modeled.
* IRQ delivery is synthetic.
* PLIC untouched.
* No production driver claim.

Before starting:

```bash
git status
git push origin main
git push origin --tags
```

If remote is ahead:

```bash
git pull --rebase origin main
./scripts/verify_all.sh
git push origin main
git push origin --tags
```

If push or GitHub Actions cannot be completed, document the blocker in `evidence/v1.7/REPORT.md`.

---

# 1. Phase Goal

Implement `v1.7-minimal-network-service`.

The goal is to introduce a minimal, explicitly bounded, user-space network service.

This phase is not a full TCP/IP stack.

This phase is not production networking.

This phase is not internet-ready.

This phase is not secure networking.

This phase is not real hardware networking.

The goal is:

```text
AxiomRT boots to axiom>
net_driver_service starts in U-mode
net_service starts in U-mode
network device capability is explicit
network status is visible from shell
bounded packet TX/RX model exists
malformed network requests fail safely
network service crash is contained
kernel remains alive
verify_all.sh passes with 16/16 QEMU tests
```

---

# 2. Architecture Law

AxiomRT remains a microkernel.

The kernel may provide only mechanisms:

* device object identity,
* device capability lookup,
* MMIO grant,
* DMA buffer grant,
* IRQ/event delivery,
* bounded IPC,
* task scheduling,
* fault containment.

The kernel must not contain:

* network stack,
* Ethernet protocol policy,
* ARP,
* IP,
* UDP,
* TCP,
* packet routing,
* DNS,
* firewall policy,
* socket API,
* shell command policy,
* packet parser above minimal driver mechanics.

Networking policy must live in user space:

* `driver_manager`,
* `net_driver_service`,
* `net_service`,
* shell.

---

# 3. Existing Behavior Must Not Regress

All existing commands must remain working:

```text
help
version
tasks
faults
ipc
caps
memory
uptime
events
apps
app info hello
run hello
run counter
run fault_demo
run demo
bin
app load hello
app unload hello
app state hello
run loaded hello
ls
ls /etc
ls /apps
ls /bin
cat /etc/version
cat /apps/hello.manifest
cat /storage/version
storage info
storage read 0
drivers
driver info block
driver fault block
driver restart block
shutdown
```

Existing tests must not be weakened.

Existing capabilities must not silently disappear.

Apps must not receive network/device capabilities unless explicitly documented and tested.

`fault_demo` must still receive no capabilities.

---

# 4. v1.7 Scope

Allowed:

* network architecture documentation,
* virtio-net investigation,
* synthetic network device model if real virtio-net is not yet safe,
* net_driver_service,
* net_service,
* network IPC protocol,
* shell commands:

  * `net status`,
  * `net stats`,
  * `net send-test`,
  * `net rx-count`,
  * `net fault`,
  * `net restart`,
* host/parser/dashboard updates,
* QEMU tests,
* evidence archive.

Forbidden:

* full TCP/IP stack,
* sockets,
* DNS,
* TLS,
* firewall,
* internet access claim,
* production network claim,
* parsing packet protocols in kernel,
* giving apps network authority by default,
* real hardware claim,
* certification claim.

---

# 5. Task Sequence

Run tasks in order.

One task = one commit, unless a disclosed grouping is necessary for a QEMU-verifiable state.

---

## AXIOM-NET-001 — Document Network Service Architecture

Create:

```text
docs/34_NETWORK_SERVICE.md
```

Must document:

1. Why networking is user-space.
2. Difference between network driver and network service.
3. What v1.7 implements.
4. What v1.7 does not implement.
5. Packet model.
6. Network IPC protocol.
7. Capability model.
8. Fault containment.
9. Security limitations.
10. Future TCP/IP path.
11. Future real hardware path.
12. Kernel boundary.

Required honesty:

State clearly:

```text
v1.7 does not implement a production network stack.
v1.7 does not implement TCP/IP.
v1.7 does not provide internet connectivity guarantees.
v1.7 does not claim network security.
```

Commit:

```text
AXIOM-NET-001: document network service architecture
```

---

## AXIOM-NET-002 — Document Virtio-Net Investigation

Create:

```text
docs/35_VIRTIO_NET_INVESTIGATION.md
```

Must document:

1. QEMU virtio-net options considered.
2. Whether AxiomRT currently uses MMIO or PCI virtio path.
3. Device discovery requirements.
4. Required MMIO registers.
5. Required virtqueue/DMA concepts.
6. Required IRQ path.
7. What current v1.5 device mechanisms can support.
8. What is still missing.
9. Whether v1.7 uses real virtio-net, modeled virtio-net, or synthetic packet service.
10. Why the selected approach does not violate the architecture law.

If real virtio-net is not implemented, say so directly.

Commit:

```text
AXIOM-NET-002: document virtio-net investigation
```

---

## AXIOM-NET-003 — Define Network IPC Protocol

Define bounded IPC protocol between shell, net_service, and net_driver_service.

Shell to net_service examples:

```text
NET_STATUS
NET_STATS
NET_SEND_TEST
NET_RX_COUNT
NET_FAULT
NET_RESTART
```

net_service to net_driver_service examples:

```text
DRV_STATUS
DRV_TX_TEST
DRV_RX_COUNT
DRV_FAULT
DRV_RESTART
```

Expected responses:

```text
OK net state=up driver=running tx=<n> rx=<n>
OK sent test_packet bytes=<n>
OK rx_count=<n>
ERR denied
ERR malformed
ERR driver_down
ERR too_large
ERR unsupported
```

Rules:

* bounded messages only,
* no dynamic allocation unless already safe and justified,
* malformed requests fail safely,
* unknown command returns `ERR malformed` or `ERR unsupported`,
* no kernel packet parsing.

Commit:

```text
AXIOM-NET-003: define network IPC protocol
```

---

## AXIOM-NET-004 — Add Network Capability Rights

Add rights:

```text
net_status
net_tx
net_rx
net_control
```

Rules:

* deny-by-default,
* shell gets status/control through net_service, not direct device access,
* net_service gets authority to communicate with net_driver_service,
* net_driver_service gets device/network driver capabilities,
* apps get no network authority by default,
* `fault_demo` gets no network authority.

If `CAPS_PER_TASK` must increase again, do it explicitly.

Add tests ensuring existing shell capabilities remain:

```text
line
console
info
control
app
fs
storage
driver
network
```

Commit:

```text
AXIOM-NET-004: add network capability rights
```

---

## AXIOM-NET-005 — Add net_driver_service Skeleton

Create U-mode `net_driver_service`.

Responsibilities:

* start in its own address space,
* receive network device capability,
* receive modeled or real MMIO/DMA/IRQ capability if used,
* expose driver status,
* maintain TX/RX counters,
* handle `DRV_TX_TEST`,
* handle `DRV_RX_COUNT`,
* support deliberate fault for containment test,
* never parse high-level network protocols.

For v1.7, if real virtio-net is not safe, implement a synthetic bounded packet path and document it.

Required event examples:

```text
NET_DRIVER started=net_driver_service
NET_DRIVER tx_test bytes=<n>
NET_DRIVER rx_count=<n>
```

Commit:

```text
AXIOM-NET-005: add net driver service skeleton
```

---

## AXIOM-NET-006 — Add net_service

Create U-mode `net_service`.

Responsibilities:

* receive shell requests,
* communicate with net_driver_service,
* expose net status/stats,
* return bounded responses,
* never access device MMIO directly,
* never parse arbitrary packets in kernel,
* fail safely if driver is down.

Expected shell-visible response:

```text
net state=up driver=running tx=0 rx=0 mode=synthetic
```

or if a real device path is used:

```text
net state=up driver=running tx=0 rx=0 mode=virtio-net-mmio
```

Commit:

```text
AXIOM-NET-006: add network service
```

---

## AXIOM-NET-007 — Wire init_service and driver_manager

Update boot flow:

* `init_service` starts `net_driver_service` and `net_service`.
* `driver_manager` tracks `net_driver_service`.
* supervisor/logger receives fault events for net services.
* service table capacity is reviewed explicitly.

Expected boot evidence:

```text
SERVICE started=net_driver_service
SERVICE started=net_service
```

Commit:

```text
AXIOM-NET-007: start network services during boot
```

---

## AXIOM-NET-008 — Add Shell Network Commands

Add shell commands:

```text
net status
net stats
net send-test
net rx-count
net fault
net restart
```

Expected:

```text
net status
-> OK net state=up driver=running ...

net send-test
-> OK sent test_packet bytes=<n>

net rx-count
-> OK rx_count=<n>

net fault
-> deliberate service fault, contained

net restart
-> restarted
```

Preserve all existing commands.

Commit:

```text
AXIOM-NET-008: add shell network commands
```

---

## AXIOM-NET-009 — Add Network Fault Containment

Goal:

Prove network service/driver failure does not kill kernel or shell.

Required behavior:

* `net fault` faults net_driver_service or net_service according to documented test path.
* kernel contains the user fault.
* supervisor receives fault.
* driver_manager observes failure.
* `net restart` restarts network driver/service.
* shell remains alive.
* existing apps/fs/storage/driver commands still work after network failure.

Expected evidence lines:

```text
FAULT type=<...> task=net_driver_service
CONTAIN scope=user reason=<...> kernel=alive
NET_DRIVER state=faulted
NET_DRIVER restarted=net_driver_service
```

Commit:

```text
AXIOM-NET-009: contain and restart network service faults
```

---

## AXIOM-NET-010 — Add Network QEMU Test

Create:

```text
tests/network_service_qemu_test.sh
```

Test must assert:

1. boot reaches `axiom>`,
2. `net status` works,
3. `net stats` works,
4. `net send-test` works,
5. `net rx-count` works,
6. malformed network command fails safely,
7. `net fault` is contained,
8. `net restart` works,
9. shell remains alive,
10. `run hello` still works,
11. `ls` still works,
12. `storage info` still works,
13. `drivers` still works,
14. `app load hello` still works,
15. `shutdown` exits QEMU 0.

Commit:

```text
AXIOM-NET-010: add network service QEMU test
```

---

## AXIOM-NET-011 — Integrate Into verify_all

Update:

```text
scripts/verify_all.sh
```

Expected:

```text
16/16 QEMU tests
VERIFY ALL: PASS
```

Must preserve:

* host tests,
* axiomctl tests,
* studio tests,
* supervisor tests,
* Coq compilation,
* zero warnings,
* clippy clean.

Commit:

```text
AXIOM-NET-011: integrate network test into verification sweep
```

---

## AXIOM-NET-012 — Update axiomctl and Studio

Update event parsing and dashboard minimally.

axiomctl should recognize/summarize:

```text
NET_DRIVER
NET_SERVICE
NET_TX
NET_RX
NET_DENIED
```

Studio should show:

* network service state,
* TX/RX counters,
* network fault/restart events,
* synthetic vs real mode,
* limitations.

Commit:

```text
AXIOM-NET-012: update host tools for network events
```

---

## AXIOM-NET-013 — Archive v1.7 Evidence

Create:

```text
evidence/v1.7/REPORT.md
evidence/v1.7/network_service_qemu_test.log
evidence/v1.7/verify_all.log
evidence/v1.7/tool_versions.txt
```

Report must state:

1. what v1.7 demonstrates,
2. whether network is synthetic or real virtio-net,
3. what the kernel does and does not do,
4. what remains unimplemented,
5. no TCP/IP claim,
6. no internet connectivity claim,
7. no production network claim,
8. no certification claim,
9. next phase.

Update README current milestone.

Tag:

```bash
git tag -a v1.7-minimal-network-service -m "AxiomRT v1.7 minimal network service"
```

Commit:

```text
AXIOM-NET-013: archive v1.7 network service evidence
```

---

# 6. Required Final Verification

Before tagging v1.7, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
./tests/network_service_qemu_test.sh
```

Expected final state:

```text
VERIFY ALL: PASS
16/16 QEMU tests
zero warnings
clippy clean
```

---

# 7. Definition of Done

v1.7 is complete only when:

* network architecture is documented,
* virtio-net investigation is documented,
* network protocol is documented,
* network capabilities exist,
* net_driver_service starts in U-mode,
* net_service starts in U-mode,
* shell commands work:

  * `net status`,
  * `net stats`,
  * `net send-test`,
  * `net rx-count`,
  * `net fault`,
  * `net restart`,
* network fault is contained,
* network restart works,
* shell remains alive,
* existing apps still work,
* existing filesystem still works,
* existing storage still works,
* existing driver framework still works,
* no network stack is placed in kernel,
* no TCP/IP claim is made,
* no production network claim is made,
* QEMU network test passes,
* verify_all passes,
* evidence archived,
* README updated,
* tag exists.

---

# 8. Forbidden Shortcuts

Do not:

* implement TCP/IP in kernel,
* implement sockets,
* claim internet support,
* give network capabilities to apps by default,
* give network capabilities to `fault_demo`,
* silently increase task/capacity constants without tests,
* hide synthetic network limitations,
* remove existing commands,
* weaken old tests,
* create network dependencies that break deterministic QEMU tests,
* claim real hardware networking,
* claim certification.

---

# 9. Next Phase After v1.7

After v1.7, the next phase is:

```text
v1.8-robustness-fuzzing
```

Do not start v1.8 until v1.7 gate passes.
