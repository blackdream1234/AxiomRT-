# AxiomRT RISC-V 64 trap entry/exit (AXIOM-TRAP-001, reworked by
# AXIOM-USER-002 for traps from user mode).
# Requirement reference: docs/10_TRAP_MODEL.md, docs/10_USER_MODE.md.
#
# sscratch discipline (docs/10_USER_MODE.md §3):
#   * kernel context: sscratch = 0
#   * user context:   sscratch = kernel trap stack top
# The trap vector distinguishes the origin by swapping sp with sscratch:
# a zero result means the trap came from kernel code (same stack); a
# nonzero result is the kernel trap stack to switch to.
#
# Trap frame layout (must match TrapFrame in trap.rs, #[repr(C)]):
#   offset  8*(N-1) : general register xN, for N = 1..31
#   offset  8*31    : sepc
#   offset  8*32    : sstatus
#   total: 272 bytes (33 slots rounded to 16-byte alignment).

.equ FRAME_SIZE, 272
.equ SSTATUS_SPP, 0x100

.section .text
.align 2                       # stvec requires 4-byte alignment
.global __trap_vector
__trap_vector:
    csrrw   sp, sscratch, sp   # sp <-> sscratch
    bnez    sp, .Lfrom_user

    # --- Trap from kernel context: sscratch held 0. Undo the swap to
    # recover the kernel sp, keep sscratch = 0.
    csrrw   sp, sscratch, sp
    addi    sp, sp, -FRAME_SIZE
    sd      x5,   4*8(sp)      # free t0 as scratch
    addi    t0, sp, FRAME_SIZE # pre-trap kernel sp
    sd      t0,   1*8(sp)
    j       .Lsave_rest

.Lfrom_user:
    # --- Trap from user context: sp = kernel trap stack top,
    # sscratch = user sp. Mark kernel context, record the user sp.
    addi    sp, sp, -FRAME_SIZE
    sd      x5,   4*8(sp)      # user's t0
    csrr    t0, sscratch       # user sp
    csrw    sscratch, x0       # now in kernel context
    sd      t0,   1*8(sp)

.Lsave_rest:
    sd      x1,   0*8(sp)
    sd      x3,   2*8(sp)
    sd      x4,   3*8(sp)
    # x5 saved above
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

    csrr    t0, sepc
    sd      t0,  31*8(sp)
    csrr    t0, sstatus
    sd      t0,  32*8(sp)

    # Rust handler: trap_handler(&mut TrapFrame). It may rewrite sepc,
    # sstatus, and the saved registers (syscall results, containment
    # redirection).
    mv      a0, sp
    call    trap_handler

    ld      t0,  31*8(sp)
    csrw    sepc, t0
    ld      t0,  32*8(sp)
    csrw    sstatus, t0

    # Returning to user? Re-arm sscratch with the kernel trap stack top
    # (this frame's base). Returning to kernel keeps sscratch = 0.
    andi    t0, t0, SSTATUS_SPP
    bnez    t0, .Lrestore
    addi    t0, sp, FRAME_SIZE
    csrw    sscratch, t0

.Lrestore:
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
    # x2 last: switches to the destination stack (kernel or user).
    ld      x2,   1*8(sp)
    sret
