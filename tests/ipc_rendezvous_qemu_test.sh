#!/bin/sh
# AxiomRT on-target IPC rendezvous QEMU test (AXIOM-IPCRT-010).
# Requirement reference: docs/17_IPC_ONTARGET.md, docs/14_TEST_STRATEGY.md.
#
# Builds the IPC demo, boots it, and asserts a synchronous message
# exchange between two U-mode tasks in separate address spaces: the
# receiver blocks, the sender sends, the message is delivered across the
# address spaces, both tasks exit, and the kernel does not panic.
# Restores the default build afterwards.
#
# Usage: ./tests/ipc_rendezvous_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_ipc.XXXXXX.log)"
cargo build --release --features demo_ipc >/dev/null 2>&1

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
    "TASK_STARTED task=receiver" \
    "TASK_STARTED task=sender" \
    "IPC recv task=receiver" \
    "state=blocked" \
    "IPC send task=sender" \
    "IPC delivered bytes=4" \
    "TASK_EXITED task=sender" \
    "TASK_EXITED task=receiver"; do
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
    echo "FAIL: IPC rendezvous test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: IPC rendezvous test"
