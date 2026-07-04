#!/bin/sh
# AxiomRT boot smoke test (AXIOM-BOOT-005).
# Requirement reference: docs/14_TEST_STRATEGY.md, docs/09_BUILD_AND_BOOT.md.
#
# First mandatory test of the project: boots the kernel in QEMU and
# verifies the serial output contains the exact boot banner. Fails if any
# banner line is missing. No OS feature is exercised beyond boot.
#
# Usage: ./tests/boot_smoke_test.sh
# Exit code: 0 = banner found (PASS), 1 = banner missing (FAIL).

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
LOG="$(mktemp /tmp/axiomrt_boot_smoke.XXXXXX.log)"
BOOT_TIMEOUT_S=15

cd "$REPO_ROOT"
cargo build --release

# The kernel halts in a wfi loop after the banner, so QEMU never exits by
# itself: bound the run with timeout. Exit code 124 (timeout) is expected.
set +e
timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
    -machine virt \
    -smp 1 \
    -m 128M \
    -nographic \
    -bios default \
    -kernel "$KERNEL_ELF" \
    < /dev/null > "$LOG" 2>&1
qemu_status=$?
set -e

if [ "$qemu_status" -ne 124 ] && [ "$qemu_status" -ne 0 ]; then
    echo "FAIL: QEMU did not run correctly (exit $qemu_status). Log: $LOG"
    exit 1
fi

fail=0
for line in "AxiomRT kernel booted" "arch=riscv64" "phase=boot"; do
    if grep -q "$line" "$LOG"; then
        echo "ok: found \"$line\""
    else
        echo "FAIL: missing banner line \"$line\""
        fail=1
    fi
done

if [ "$fail" -ne 0 ]; then
    echo "FAIL: boot banner incomplete. Full serial log: $LOG"
    exit 1
fi

rm -f "$LOG"
echo "PASS: boot smoke test"
