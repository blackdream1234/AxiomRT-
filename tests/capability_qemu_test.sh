#!/bin/sh
# AxiomRT on-target capability enforcement QEMU test (AXIOM-CAPRT-008).
# Requirement reference: docs/18_CAP_ONTARGET.md, docs/14_TEST_STRATEGY.md.
#
# Builds the capability demo, boots it, and asserts that a send from a
# task without a valid capability is denied with the endpoint unchanged,
# that a send from a task holding the Send capability then delivers, and
# that the kernel does not panic. Restores the default build afterwards.
#
# Usage: ./tests/capability_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_cap.XXXXXX.log)"
cargo build --release --features demo_cap >/dev/null 2>&1

set +e
timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel "$KERNEL_ELF" < /dev/null > "$log" 2>&1
qs=$?
set -e

cargo build --release >/dev/null 2>&1

if [ "$qs" -ne 124 ] && [ "$qs" -ne 0 ]; then
    echo "FAIL: QEMU exit $qs (log: $log)"
    exit 1
fi

fail=0
for pat in \
    "TASK_STARTED task=faulty_task" \
    "CAP_DENIED task=faulty_task reason=no_valid_capability" \
    "IPC state=unchanged" \
    "IPC send task=good_sender" \
    "IPC delivered bytes=4"; do
    if grep -q "$pat" "$log"; then
        echo "ok: $pat"
    else
        echo "FAIL: missing \"$pat\""
        fail=1
    fi
done

if grep -q "PANIC" "$log"; then
    echo "FAIL: kernel panicked (log: $log)"
    fail=1
else
    echo "ok: no PANIC"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: capability enforcement test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: capability enforcement test"
