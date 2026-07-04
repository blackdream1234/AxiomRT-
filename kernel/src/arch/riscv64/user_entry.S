# AxiomRT first user-mode transition (AXIOM-USER-002).
# Requirement reference: docs/10_USER_MODE.md.
#
# __enter_user(entry a0, user_sp a1, trap_stack_top a2) -> !
#
# Performs the one controlled S->U transition of Phase 7:
#   * sscratch := kernel trap stack top (nonzero = "user context" marker
#     for the trap vector, docs/10_USER_MODE.md §3)
#   * sepc     := user entry point
#   * sstatus.SPP := 0 (sret drops to user privilege)
#   * sp       := user stack
#   * sret
#
# Never returns. Any user trap re-enters the kernel exclusively through
# __trap_vector (trap.S); a bad return path cannot exist because there
# is no return address — only the trap path leads back.

.equ SSTATUS_SPP, 0x100

.section .text
.global __enter_user
__enter_user:
    csrw    sscratch, a2
    csrw    sepc, a0
    li      t0, SSTATUS_SPP
    csrc    sstatus, t0
    mv      sp, a1
    sret
