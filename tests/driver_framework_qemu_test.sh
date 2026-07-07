#!/bin/sh
# Driver framework test (AXIOM-DRV-009).
# Requirement reference: docs/31_USER_SPACE_DRIVER_FRAMEWORK.md,
# AxiomRT v1.5.md §5.
#
# Asserts the v1.5 gate: boot reaches the shell, the driver framework
# comes up (device registered, MMIO/DMA grants, synthetic IRQ
# delivered), `drivers`/`driver info block` answer, `driver fault
# block` is a contained user fault observed by supervisor and
# driver_manager, `driver restart block` recovers the driver, the
# shell survives, existing app/storage behavior is unchanged, and
# shutdown is controlled.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted driver session)"
(
    sleep 4
    printf 'drivers\r'
    sleep 1
    printf 'driver info block\r'
    sleep 1
    printf 'driver fault block\r'
    sleep 3
    printf 'drivers\r'
    sleep 1
    printf 'driver restart block\r'
    sleep 2
    printf 'drivers\r'
    sleep 1
    printf 'caps\r'
    sleep 1
    printf 'run hello\r'
    sleep 2
    printf 'run fault_demo\r'
    sleep 3
    printf 'cat /storage/version\r'
    sleep 1
    printf 'shutdown\r'
    sleep 3
) | timeout 90 qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel target/riscv64gc-unknown-none-elf/release/kernel \
    >"$LOG" 2>&1
QEMU_EXIT=$?

fail=0
expect() {
    if grep -q "$1" "$LOG"; then
        echo "ok: found \"$1\""
    else
        echo "MISSING: \"$1\""
        fail=1
    fi
}

# 1. Boot reaches the shell.
expect "axiom> "

# Framework bring-up evidence (docs/31 §6-§9).
expect "DEVICE registered=block0 kind=block_skeleton"
expect "IRQ registered source=block0 endpoint=driver_irq"
expect "MMIO grant task=block_driver_service device=block0 region=virtio_mmio0"
expect "DMA grant task=block_driver_service buffer=block0_dma size=4096"
expect "DRIVER started=block_driver_service"
# Real MMIO: the virtio magic register reads 'virt'.
expect "MMIO read task=block_driver_service device=block0 offset=0 value=0x74726976"
# Withheld right is denied (mmio_write is granted to no v1.5 task).
expect "MMIO_DENIED task=block_driver_service reason=insufficient_rights"
# Synthetic boot attention event delivered to the driver.
expect "IRQ delivered to=block_driver_service source=block0"

# 2./3. drivers lists the driver; driver info block answers.
expect "driver name=block_driver_service state=running kind=block_skeleton"
expect "block_driver_service running"
expect "kind=block_skeleton state=running mmio=granted irq=registered"

# 4./5. driver fault block: contained user fault, supervisor notified.
expect "FAULT type=PageFault task=block_driver_service"
expect "CONTAIN scope=user reason=user_access_unmapped action=faulted kernel=alive"
expect "IPC delivered fault_event to=supervisor_task from=block_driver_service"

# 6. driver_manager observes the failure (IRQ probe + tracked state).
expect "IRQ_DROPPED reason=driver_not_ready"
expect "DRIVER_MANAGER observed=fault driver=block_driver_service"
expect "block_driver_service faulted"

# 7./8. restart recovers the driver; drivers shows it running again.
expect "TASK_RESTARTED task=block_driver_service"
expect "DRIVER restarted=block_driver_service"

# 9. Shell alive: full capability set intact (8/8, docs/31 §10).
expect "caps task=shell_service endpoint console info control endpoint endpoint endpoint endpoint"

# 10. Existing apps still work; fault_demo still owns nothing.
expect "hello from app: hello"
expect "MMIO_DENIED task=fault_demo reason=no_valid_capability"
expect "DMA_DENIED task=fault_demo reason=no_valid_capability"

# 11. Storage chain unchanged.
expect "OK data=AxiomRT v1.6"

# 12. Controlled shutdown.
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi
if grep -q "PANIC kernel=axiomrt" "$LOG"; then
    echo "MISSING: kernel must not fault during driver containment"
    fail=1
else
    echo "ok: no kernel panic"
fi

# The driver must come back after the fault: 'running' must appear
# again AFTER the 'faulted' state line (order check, assertion 8).
if awk '/block_driver_service faulted/{f=1} f && /driver name=block_driver_service state=running/{ok=1} END{exit ok?0:1}' "$LOG"; then
    echo "ok: driver running again after fault"
else
    echo "MISSING: running state after fault"
    fail=1
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: driver framework test"
else
    echo "FAIL: driver framework test"
    sed -n '1,260p' "$LOG"
fi
exit "$fail"
