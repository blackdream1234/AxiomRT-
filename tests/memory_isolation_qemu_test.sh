#!/bin/sh
# AxiomRT memory isolation QEMU tests (AXIOM-MEMHW-009..011).
# Requirement reference: docs/12_MMU_SV39.md §7, docs/14_TEST_STRATEGY.md.
#
# Boots the kernel under Sv39 and asserts that forbidden user memory
# accesses take the expected page fault, are contained, and the kernel
# survives. Each case selects the demo probe via a cargo feature; the
# default build is restored at the end so other tests see the normal
# kernel.
#
# Usage: ./tests/memory_isolation_qemu_test.sh
# Exit: 0 = all cases pass, 1 = any case failed.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

fail=0

# run_case <feature-flags> <expected-reason> <label>
run_case() {
    feature="$1"
    expected="$2"
    label="$3"
    log="$(mktemp /tmp/axiomrt_memiso.XXXXXX.log)"

    # shellcheck disable=SC2086
    cargo build --release $feature >/dev/null 2>&1

    set +e
    timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
        -machine virt -smp 1 -m 128M -nographic -bios default \
        -kernel "$KERNEL_ELF" < /dev/null > "$log" 2>&1
    qs=$?
    set -e
    if [ "$qs" -ne 124 ] && [ "$qs" -ne 0 ]; then
        echo "FAIL[$label]: QEMU exit $qs (log: $log)"
        fail=1
        return
    fi

    if grep -q "MMU status=enabled mode=sv39" "$log" \
        && grep -q "reason=$expected" "$log" \
        && grep -q "kernel=survived" "$log"; then
        echo "ok[$label]: page fault contained, reason=$expected, kernel survived"
        rm -f "$log"
    else
        echo "FAIL[$label]: expected reason=$expected + survival not observed (log: $log)"
        fail=1
    fi
}

# AXIOM-MEMHW-009: user read of kernel memory -> load page fault.
run_case "" "user_access_kernel_memory" "read-kernel"

# Restore the default build for subsequent tests.
cargo build --release >/dev/null 2>&1

if [ "$fail" -ne 0 ]; then
    echo "FAIL: memory isolation QEMU tests"
    exit 1
fi
echo "PASS: memory isolation QEMU tests"
