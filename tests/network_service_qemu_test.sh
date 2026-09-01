#!/bin/sh
# Minimal network service test (AXIOM-NET-010).
# Requirement reference: docs/34_NETWORK_SERVICE.md.
#
# Exercises only deterministic synthetic packet mode. It proves bounded
# user-space service/driver IPC, deliberate driver fault containment,
# restart, and preservation of existing shell/app/fs/storage/driver paths.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted network-service session)"
(
    sleep 4
    printf 'net status\r'
    sleep 1
    printf 'net stats\r'
    sleep 1
    printf 'net send-test\r'
    sleep 1
    printf 'net rx-count\r'
    sleep 1
    printf 'net malformed\r'
    sleep 1
    printf 'net fault\r'
    sleep 4
    printf 'net status\r'
    sleep 1
    printf 'net restart\r'
    sleep 3
    printf 'net status\r'
    sleep 1
    printf 'caps\r'
    sleep 1
    printf 'run hello\r'
    sleep 2
    printf 'ls\r'
    sleep 1
    printf 'storage info\r'
    sleep 1
    printf 'drivers\r'
    sleep 1
    printf 'app load hello\r'
    sleep 1
    printf 'shutdown\r'
    sleep 3
) | timeout 100 qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel target/riscv64gc-unknown-none-elf/release/kernel \
    >"$LOG" 2>&1
QEMU_EXIT=$?

fail=0
expect() {
    if grep -Fq "$1" "$LOG"; then
        echo "ok: found \"$1\""
    else
        echo "MISSING: \"$1\""
        fail=1
    fi
}

# 1. Boot reaches the shell and both isolated services start.
expect "axiom> "
expect "SERVICE started=net_driver_service"
expect "SERVICE started=net_service"
expect "NET_DRIVER started=net_driver_service"
expect "NET_SERVICE state=up mode=synthetic"

# 2. Network status reports the explicit synthetic mode.
expect "OK net state=up driver=running tx=0 rx=0 mode=synthetic"

# 3. Counter-only stats work.
expect "OK tx=0 rx=0"

# 4. One fixed-size synthetic packet is sent.
expect "OK sent test_packet bytes=64"
expect "NET_DRIVER tx_test bytes=64"
expect "NET_TX bytes=64 tx=1"
expect "NET_RX rx=1"

# 5. RX count reflects deterministic synthetic loopback.
expect "OK rx_count=1"
expect "NET_DRIVER rx_count=1"

# 6. Malformed network input is bounded and rejected by net_service.
expect "NET_DENIED reason=malformed"
expect "ERR malformed"

# 7. The deliberate network-driver U-mode fault is contained.
expect "FAULT type=PageFault task=net_driver_service"
expect "CONTAIN scope=user reason=user_access_unmapped action=faulted kernel=alive"
expect "IPC delivered fault_event to=supervisor_task from=net_driver_service"
expect "DRIVER_MANAGER observed=fault driver=net_driver_service"
expect "NET_DRIVER state=faulted"
expect "OK network fault contained"
expect "ERR driver_down"

# 8. Manager restart restores the isolated driver and resets counters.
expect "TASK_RESTARTED task=net_driver_service"
expect "NET_DRIVER restarted=net_driver_service"
expect "OK restarted"

# 9. Shell and network service remain alive after restart.
if awk '/NET_DRIVER restarted=net_driver_service/{r=1} r && /OK net state=up driver=running tx=0 rx=0 mode=synthetic/{ok=1} END{exit ok?0:1}' "$LOG"; then
    echo "ok: network status recovered after restart"
else
    echo "MISSING: recovered network status after restart"
    fail=1
fi

# 10-14. Existing app, fs, storage, driver, and loader paths survive.
expect "hello from app: hello"
expect "OK etc apps docs bin"
expect "OK block_size=48 blocks=8 readonly=true"
expect "driver name=block_driver_service state=running kind=block_skeleton"
expect "driver name=net_driver_service state=running kind=network_synthetic"
expect "OK loaded hello"

# Shell keeps the original eight grants plus the service-only network cap.
expect "caps task=shell_service endpoint console info control endpoint endpoint endpoint endpoint endpoint"

# 15. Controlled shutdown exits QEMU successfully.
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi

if grep -Fq "PANIC kernel=axiomrt" "$LOG"; then
    echo "MISSING: kernel must not fault during network containment"
    fail=1
else
    echo "ok: no kernel panic"
fi
if grep -Fq "FAULT type=PageFault task=net_service" "$LOG"; then
    echo "MISSING: net_service itself must stay alive"
    fail=1
else
    echo "ok: net_service stayed alive"
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: network service test"
else
    echo "FAIL: network service test"
    sed -n '1,360p' "$LOG"
fi
exit "$fail"
