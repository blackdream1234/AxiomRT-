//! AxiomRT user task layer (Phase 7).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md, docs/10_USER_MODE.md.
//!
//! AXIOM-USER-001: the user image *model* — descriptors that say what a
//! user task is (entry, stack region, address space).
//! AXIOM-USER-002: the first controlled S→U transition and the trap-path
//! return, including fault containment (kernel survives user faults).

pub mod image;

pub use image::{ImageError, UserImage};

#[cfg(target_arch = "riscv64")]
pub use run::first_user_task_demo;

/// Runtime user-mode entry (riscv64 target only).
#[cfg(target_arch = "riscv64")]
mod run {
    use kernel::memory::address::{VirtAddr, PAGE_SIZE};
    use kernel::memory::AddressSpaceId;

    use super::UserImage;
    use crate::trap;
    use crate::uart;

    core::arch::global_asm!(include_str!("../arch/riscv64/user_entry.S"));

    extern "C" {
        /// S→U transition (arch/riscv64/user_entry.S). Never returns;
        /// the only way back into the kernel is the trap path.
        fn __enter_user(entry: u64, user_sp: u64, trap_stack_top: u64) -> !;
        /// Boot stack top from kernel/linker.ld; reused as the kernel
        /// continuation stack after the user demo terminates.
        static __stack_top: u8;
    }

    /// Dedicated kernel stack for traps taken from user mode (the boot
    /// stack still holds the suspended kernel_main frame).
    #[repr(C, align(16))]
    struct TrapStack([u8; 8 * 1024]);
    static mut USER_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

    /// Stack for the demo user task. Pre-MMU interim: physically in
    /// kernel RAM (docs/10_USER_MODE.md §5 documents this limitation).
    #[repr(C, align(16))]
    struct UserStack([u8; 4 * 1024]);
    static mut DEMO_USER_STACK: UserStack = UserStack([0; 4 * 1024]);

    /// The first user task. Runs at privilege U: it can only ecall and
    /// fault — every privileged operation traps to the kernel.
    extern "C" fn demo_user_task() -> ! {
        // 1. Syscall round trip: sys_yield then sys_exit (stubs), each
        //    returning through the trap path.
        // SAFETY: `ecall` from U-mode is the architecturally defined
        // syscall entry (docs/04_SYSCALL_MODEL.md ABI); the kernel trap
        // path saves/restores all registers except a0 (result), which
        // is declared clobbered here along with a7 (number).
        unsafe {
            core::arch::asm!("li a7, 1", "ecall", out("a7") _, out("a0") _);
            core::arch::asm!("li a7, 2", "ecall", out("a7") _, out("a0") _);
        }
        // 2. Deliberate fault: reading a privileged CSR from U-mode
        //    raises IllegalInstruction. The kernel must contain it and
        //    survive (AXIOM-USER-002 definition of done).
        // SAFETY: this instruction is *meant* to trap; it never
        // completes, so no register state escapes.
        unsafe {
            core::arch::asm!("csrr t0, sstatus", out("t0") _);
        }
        // Unreachable: the fault above terminates the task.
        loop {
            core::hint::spin_loop();
        }
    }

    /// Kernel continuation after the user task terminates (fault
    /// containment redirect target, docs/10_USER_MODE.md §4).
    extern "C" fn user_demo_done() -> ! {
        uart::put_str("USER demo=first_user_task result=contained kernel=survived\n");
        uart::put_str("phase=user-demo-complete\n");
        loop {
            core::hint::spin_loop();
        }
    }

    /// Enter the first user task (AXIOM-USER-002).
    ///
    /// Also validates the v0.1 *virtual* layout spec of the first user
    /// task through the UserImage model (AXIOM-USER-001). Pre-MMU, the
    /// physical execution addresses differ from that virtual plan; the
    /// model check documents and pins the target layout.
    pub fn first_user_task_demo() -> ! {
        let spec = UserImage::new(
            VirtAddr::new(0x1_0000),
            VirtAddr::new(0x20_0000),
            PAGE_SIZE,
            AddressSpaceId(1),
        );
        if spec.is_err() {
            uart::put_str("PANIC kernel=axiomrt reason=invalid_user_image_spec\n");
            loop {
                core::hint::spin_loop();
            }
        }

        // Where a faulting/terminated user task returns the kernel to:
        // __stack_top is the linker-script boot stack top
        // (kernel/linker.ld). The suspended kernel_main frame below it
        // is intentionally abandoned — the continuation never returns.
        // (addr_of! creates raw pointers without dereferencing: safe.)
        let cont_sp = core::ptr::addr_of!(__stack_top) as u64;
        trap::set_user_fault_continuation(user_demo_done as *const () as u64, cont_sp);

        // Static stack addresses are taken once, before user mode
        // starts, and stay valid forever (statics never move). Tops
        // point one-past-the-end, 16-byte aligned per repr(align).
        let trap_stack_top = core::ptr::addr_of!(USER_TRAP_STACK) as u64 + 8 * 1024;
        let user_stack_top = core::ptr::addr_of!(DEMO_USER_STACK) as u64 + 4 * 1024;

        uart::put_str("USER enter=demo_task mode=U isolation=privilege\n");
        // SAFETY: __enter_user performs the architecturally defined
        // S→U transition (sscratch := trap stack, sepc := entry,
        // SPP := 0, sret) with a valid entry point and stacks
        // established above; it never returns and every re-entry goes
        // through __trap_vector (docs/10_USER_MODE.md §3).
        unsafe {
            __enter_user(
                demo_user_task as *const () as u64,
                user_stack_top,
                trap_stack_top,
            )
        }
    }
}
