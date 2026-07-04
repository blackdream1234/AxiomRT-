# AxiomRT RISC-V 64 boot entry (AXIOM-BOOT-002).
# Requirement reference: docs/02_KERNEL_BLUEPRINT.md §5, docs/09_BUILD_AND_BOOT.md.
#
# OpenSBI enters here in supervisor mode with:
#   a0 = hart id
#   a1 = device tree blob physical address
#
# Responsibilities (Phase 2 scope only):
#   1. park all harts except hart 0 (no multicore in v0.1)
#   2. set up the boot stack
#   3. clear .bss
#   4. call kernel_main (never returns)

.section .text.boot
.global _start
_start:
    # v0.1 is single-hart: park everything except hart 0.
    bnez    a0, .Lpark

    # Boot stack (region reserved in kernel/linker.ld).
    la      sp, __stack_top

    # Clear .bss so Rust statics start zeroed as the language requires.
    la      t0, __bss_start
    la      t1, __bss_end
.Lbss_clear:
    bgeu    t0, t1, .Lbss_done
    sd      zero, 0(t0)
    addi    t0, t0, 8
    j       .Lbss_clear
.Lbss_done:

    # Rust entry: kernel_main(hartid, dtb). Must never return.
    call    kernel_main

.Lpark:
    # Secondary harts and any impossible return: halt safely.
    wfi
    j       .Lpark
