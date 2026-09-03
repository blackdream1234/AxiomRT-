# AxiomRT v1.7 Evidence Report — Minimal Network Service

Phase: AXIOM-NET (design: docs/34 and docs/35).
Intended tag: `v1.7-minimal-network-service` (not created in this archive).
Archived: 2026-09-03. Tool versions: `tool_versions.txt`.
Status: release candidate; final QEMU and Coq gates are pending.

## 1. What v1.7 implements

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

The implemented containment behavior is that a deliberate network-driver
U-mode fault is contained to `net_driver_service`; the shell remains alive,
`net_service` reports the driver down, and a manager-requested restart
restores the driver with counters reset. Existing application, filesystem,
storage, and unrelated driver paths remain alive. These runtime statements
are acceptance criteria in `tests/network_service_qemu_test.sh`; they could
not be executed in this Codex environment and remain pending user-machine
verification.

## 2. Verification captured here

`network_service_qemu_test.log` records an `os_boot` release build that
succeeded, followed by test failure because `qemu-system-riscv64` was not
installed (QEMU process exit 127). Its runtime assertions were therefore not
executed; missing lines in that log are not treated as observed behavior.

`verify_all.log` records the full sweep:

* 0/16 QEMU tests executed successfully because the QEMU executable was
  unavailable;
* kernel host tests passed (116 library, 4 binary, and 30 integration tests);
* axiomctl host tests passed (17);
* supervisor host tests passed (4);
* Studio host tests passed (6);
* Coq model compilation was not executed because `coqc` was unavailable;
* final result: `VERIFY ALL: FAIL`, exit code 1, due to those missing tools.

Additional available checks run before archiving:

```text
cargo fmt --check
  PASS
cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
  PASS
cargo clippy -p kernel -- -D warnings
  PASS
cargo test --target x86_64-unknown-linux-gnu -p kernel
  PASS
cargo test --target x86_64-unknown-linux-gnu -p axiomctl
  PASS (17 tests)
cargo test --target x86_64-unknown-linux-gnu -p studio
  PASS (6 tests)
```

The handoff's unqualified `cargo clippy --all-targets -- -D warnings` does
not select the host target. Because `.cargo/config.toml` deliberately makes
`riscv64gc-unknown-none-elf` the default, Cargo attempts to build test
targets that require `std` on bare metal and exits 101. This is a command/
target mismatch, not a Clippy warning. The explicit host-target workspace
command and the normal bare-kernel command above are clean.

## 3. Required user-machine release gates

Final verification must be run on the user's machine with QEMU and Coq
installed. At minimum:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p kernel -- -D warnings
./tests/network_service_qemu_test.sh
./scripts/verify_all.sh
```

The release gate is `VERIFY ALL: PASS`, `16/16 QEMU tests`, zero warnings,
and clean Clippy. Only after that result should the annotated
`v1.7-minimal-network-service` tag be created.

## 4. Explicit limitations

This is synthetic networking only. v1.7 has no TCP/IP, sockets, Ethernet,
ARP, UDP, TCP, DNS, routing, or TLS. It makes no internet-connectivity claim,
no production-network claim, no real-hardware-networking claim, and no
network-security claim. It does not implement virtio-net, device DMA, or
real network interrupts.

There is no safety or certification claim. The repository remains an
emulator-oriented evaluation system; this archive is not DO-178C,
ISO 26262, IEC 61508, or equivalent certification evidence.

## 5. Next phase

After the v1.7 user-machine gates pass and the release is tagged, the next
planned phase is `v1.8-robustness-fuzzing`. This archive does not begin that
phase.
