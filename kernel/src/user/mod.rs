//! AxiomRT user task layer (Phase 7; v0.2 memory isolation).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md, docs/10_USER_MODE.md,
//! docs/12_MMU_SV39.md.
//!
//! AXIOM-USER-001: the user image *model* (entry, stack, address space).
//! AXIOM-USER-002: the first controlled S→U transition and fault
//! containment.
//! AXIOM-MEMHW-005..007: run the demo user task under its own Sv39 page
//! table so memory isolation is hardware-enforced — U-mode cannot read
//! kernel memory (it takes a page fault, contained).

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
    use crate::paging_hw;
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

    /// Physical stack frame for the demo user task, mapped into the user
    /// address space at a user virtual address (docs/12_MMU_SV39.md §5).
    #[repr(C, align(4096))]
    struct UserStack([u8; 4 * 1024]);
    static mut DEMO_USER_STACK: UserStack = UserStack([0; 4 * 1024]);

    /// A kernel address the user task tries (and fails) to read.
    const KERNEL_PROBE_ADDR: u64 = 0x8020_0000;

    /// The demo user task. Runs at privilege U under its own page table:
    /// it attempts to read kernel memory, which the MMU refuses (the
    /// page is mapped U=0). The resulting page fault is contained.
    ///
    /// Position-independent: uses only immediates and its own stack, so
    /// it runs correctly from its user virtual mapping.
    extern "C" fn demo_user_task() -> ! {
        // SAFETY: this load is *meant* to fault — a U-mode read of a
        // kernel-only page. It never completes; no state escapes. t0/t1
        // are declared clobbered.
        unsafe {
            core::arch::asm!(
                "li t0, {addr}",
                "ld t1, 0(t0)",
                addr = const KERNEL_PROBE_ADDR,
                out("t0") _,
                out("t1") _,
            );
        }
        // Unreachable: the fault above terminates the task.
        loop {
            core::hint::spin_loop();
        }
    }

    /// Kernel continuation after the user task terminates (fault
    /// containment redirect target, docs/10_USER_MODE.md §4).
    extern "C" fn user_demo_done() -> ! {
        uart::put_str("USER demo=memory_isolation result=contained kernel=survived\n");
        uart::put_str("phase=user-demo-complete\n");
        loop {
            core::hint::spin_loop();
        }
    }

    /// Enter the demo user task under a hardware-enforced user address
    /// space (AXIOM-MEMHW-005/006/007).
    pub fn first_user_task_demo() -> ! {
        // Pin the user virtual layout through the UserImage model.
        let spec = UserImage::new(
            VirtAddr::new(0x1_0000),
            VirtAddr::new(0x20_0000 + PAGE_SIZE),
            PAGE_SIZE,
            AddressSpaceId(1),
        );
        if spec.is_err() {
            uart::put_str("PANIC kernel=axiomrt reason=invalid_user_image_spec\n");
            loop {
                core::hint::spin_loop();
            }
        }

        // Kernel continuation for the contained fault.
        let cont_sp = core::ptr::addr_of!(__stack_top) as u64;
        trap::set_user_fault_continuation(user_demo_done as *const () as u64, cont_sp);

        // Physical addresses of the task's code and stack frames.
        let code_phys = demo_user_task as *const () as u64;
        let stack_phys = core::ptr::addr_of!(DEMO_USER_STACK) as u64;

        // Build the user address space (kernel maps U=0 for the trap
        // handler + user code/stack U=1) and switch to it.
        let uas = paging_hw::build_demo_user_address_space(code_phys, stack_phys);
        // SAFETY: uas.root is a freshly built Sv39 root that maps the
        // kernel regions (U=0) identity, so this code and its stack stay
        // valid across the satp switch (docs/12_MMU_SV39.md §5/§7).
        unsafe { paging_hw::switch_to_user_space(uas.root) };

        let trap_stack_top = core::ptr::addr_of!(USER_TRAP_STACK) as u64 + 8 * 1024;

        uart::put_str("USER enter=demo_task mode=U isolation=memory\n");
        // SAFETY: __enter_user performs the architecturally defined S→U
        // transition with a valid user entry VA and user stack VA mapped
        // in the active user table; it never returns and every re-entry
        // goes through __trap_vector (docs/10_USER_MODE.md §3).
        unsafe { __enter_user(uas.entry_va, uas.stack_top_va, trap_stack_top) }
    }
}
