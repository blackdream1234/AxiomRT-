/* AxiomRT kernel linker script (AXIOM-BOOT-002; section symbols added by
 * AXIOM-MEMHW-004 for Sv39 per-region permissions).
 * Requirement reference: docs/09_BUILD_AND_BOOT.md, docs/12_MMU_SV39.md.
 *
 * QEMU virt machine + OpenSBI: firmware occupies RAM from 0x80000000;
 * the supervisor-mode payload is entered at 0x80200000.
 * Layout is static; there is no heap (docs/05_MEMORY_MODEL.md §2).
 *
 * Section boundary symbols (__text_start, __text_end, ...) are exported
 * so the kernel can build an Sv39 identity map with correct per-region
 * permissions (text R+X, rodata R, data/bss/stack R+W). Each region is
 * 4 KiB aligned so page ranges are exact.
 */

OUTPUT_ARCH(riscv)
ENTRY(_start)

KERNEL_BASE = 0x80200000;
BOOT_STACK_SIZE = 64K;

SECTIONS
{
    . = KERNEL_BASE;

    .text : ALIGN(4K) {
        __text_start = .;
        /* Boot entry must be first: OpenSBI jumps to KERNEL_BASE. */
        KEEP(*(.text.boot))
        *(.text .text.*)
        . = ALIGN(4K);
        __text_end = .;
    }

    .rodata : ALIGN(4K) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        . = ALIGN(4K);
        __rodata_end = .;
    }

    /* User-region services (docs/25_OS_BOOT_FLOW.md §2): constrained
     * U-mode Rust code and constants, gathered apart from kernel
     * .text/.rodata so service address spaces can map exactly this
     * span (text U+R+X, rodata U+R) without exposing kernel pages. */
    .user : ALIGN(4K) {
        __user_text_start = .;
        *(.user.text .user.text.*)
        . = ALIGN(4K);
        __user_text_end = .;
        __user_rodata_start = .;
        *(.user.rodata .user.rodata.*)
        . = ALIGN(4K);
        __user_rodata_end = .;
    }

    /* Writable region: data, bss, and boot stack are one R+W span. */
    .data : ALIGN(4K) {
        __data_start = .;
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
        . = ALIGN(4K);
        __stack_top = .;
    }
    __data_end = .;

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
    }
}
