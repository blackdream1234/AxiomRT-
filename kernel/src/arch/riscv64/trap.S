# AxiomRT RISC-V 64 trap entry/exit (AXIOM-TRAP-001).
# Requirement reference: docs/10_TRAP_MODEL.md.
#
# Direct-mode supervisor trap vector. Phase 3 scope: traps are taken from
# kernel (S-mode) context only — there is no user mode yet — so the trap
# frame is pushed on the current kernel stack.
#
# Trap frame layout (must match TrapFrame in trap.rs, #[repr(C)]):
#   offset  8*(N-1) : general register xN, for N = 1..31 (x0 is constant)
#   offset  8*31    : sepc
#   total: 256 bytes, 16-byte aligned.

.equ FRAME_SIZE, 256

.section .text
.align 2                       # stvec requires 4-byte alignment
.global __trap_vector
__trap_vector:
    addi    sp, sp, -FRAME_SIZE

    sd      x1,   0*8(sp)
    # x2 (sp) is saved below after its pre-trap value is recomputed.
    sd      x3,   2*8(sp)
    sd      x4,   3*8(sp)
    sd      x5,   4*8(sp)
    sd      x6,   5*8(sp)
    sd      x7,   6*8(sp)
    sd      x8,   7*8(sp)
    sd      x9,   8*8(sp)
    sd      x10,  9*8(sp)
    sd      x11, 10*8(sp)
    sd      x12, 11*8(sp)
    sd      x13, 12*8(sp)
    sd      x14, 13*8(sp)
    sd      x15, 14*8(sp)
    sd      x16, 15*8(sp)
    sd      x17, 16*8(sp)
    sd      x18, 17*8(sp)
    sd      x19, 18*8(sp)
    sd      x20, 19*8(sp)
    sd      x21, 20*8(sp)
    sd      x22, 21*8(sp)
    sd      x23, 22*8(sp)
    sd      x24, 23*8(sp)
    sd      x25, 24*8(sp)
    sd      x26, 25*8(sp)
    sd      x27, 26*8(sp)
    sd      x28, 27*8(sp)
    sd      x29, 28*8(sp)
    sd      x30, 29*8(sp)
    sd      x31, 30*8(sp)

    # Pre-trap sp (kernel context, same stack).
    addi    t0, sp, FRAME_SIZE
    sd      t0,   1*8(sp)

    csrr    t0, sepc
    sd      t0,  31*8(sp)

    # Rust handler: trap_handler(&mut TrapFrame)
    mv      a0, sp
    call    trap_handler

    # sepc may have been advanced (e.g. past an ecall).
    ld      t0,  31*8(sp)
    csrw    sepc, t0

    ld      x1,   0*8(sp)
    ld      x3,   2*8(sp)
    ld      x4,   3*8(sp)
    ld      x5,   4*8(sp)
    ld      x6,   5*8(sp)
    ld      x7,   6*8(sp)
    ld      x8,   7*8(sp)
    ld      x9,   8*8(sp)
    ld      x10,  9*8(sp)
    ld      x11, 10*8(sp)
    ld      x12, 11*8(sp)
    ld      x13, 12*8(sp)
    ld      x14, 13*8(sp)
    ld      x15, 14*8(sp)
    ld      x16, 15*8(sp)
    ld      x17, 16*8(sp)
    ld      x18, 17*8(sp)
    ld      x19, 18*8(sp)
    ld      x20, 19*8(sp)
    ld      x21, 20*8(sp)
    ld      x22, 21*8(sp)
    ld      x23, 22*8(sp)
    ld      x24, 23*8(sp)
    ld      x25, 24*8(sp)
    ld      x26, 25*8(sp)
    ld      x27, 26*8(sp)
    ld      x28, 27*8(sp)
    ld      x29, 28*8(sp)
    ld      x30, 29*8(sp)
    ld      x31, 30*8(sp)

    addi    sp, sp, FRAME_SIZE
    sret
