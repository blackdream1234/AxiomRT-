/* AxiomRT kernel linker script (AXIOM-BOOT-002).
 * Requirement reference: docs/09_BUILD_AND_BOOT.md.
 *
 * QEMU virt machine + OpenSBI: firmware occupies RAM from 0x80000000;
 * the supervisor-mode payload is entered at 0x80200000.
 * Layout is static; there is no heap (docs/05_MEMORY_MODEL.md §2).
 */

OUTPUT_ARCH(riscv)
ENTRY(_start)

KERNEL_BASE = 0x80200000;
BOOT_STACK_SIZE = 64K;

SECTIONS
{
    . = KERNEL_BASE;

    .text : ALIGN(4K) {
        /* Boot entry must be first: OpenSBI jumps to KERNEL_BASE. */
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* Boot stack: grows downward from __stack_top. */
    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += BOOT_STACK_SIZE;
        __stack_top = .;
    }

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
    }
}
