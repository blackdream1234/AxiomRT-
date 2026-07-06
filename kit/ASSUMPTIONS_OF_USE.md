# AxiomRT v1.0 — Assumptions of Use

The guarantees AxiomRT demonstrates hold only within the following
assumptions. An evaluator relying on any AxiomRT property must confirm
these hold in their context.

## Platform assumptions

1. **Target.** Evaluation runs on the QEMU `virt` machine, RISC-V 64
   (RV64GC), single hart, with the OpenSBI firmware bundled with QEMU
   (`-bios default`).
2. **OpenSBI / M-mode is trusted.** The kernel runs in supervisor mode
   entered from OpenSBI. OpenSBI, the SBI TIME extension, and machine
   mode are outside the AxiomRT trusted computing base and are assumed
   correct. `ecall` from S-mode is the SBI convention handled by OpenSBI.
3. **MMU correctness.** Sv39 translation and permission enforcement by
   the (emulated) hardware are assumed correct; AxiomRT's memory
   isolation claim rests on this assumption.
4. **Timer.** The RISC-V `time` CSR and SBI `set_timer` are assumed to
   deliver supervisor timer interrupts; preemption and the watchdog
   depend on it.

## Configuration assumptions

5. **Single active task set**, defined statically at boot. Capabilities
   are minted at boot from the static task description; there is no
   runtime capability granting in v1.0.
6. **No DMA-capable device** is exposed to user tasks. User tasks reach
   devices only through kernel-mediated services; the only device in
   v1.0 is the kernel-owned UART.
7. **Supervisor policy** is configured as documented (recovery = Kill in
   the demo). The supervisor is trusted for *policy only*; it cannot
   bypass capability checks or isolation.

## Toolchain assumptions

8. Build toolchain: Rust stable with the `riscv64gc-unknown-none-elf`
   target; QEMU ≥ 7 with RISC-V system emulation; optionally Coq 8.20
   for the proof models. Versions used for this kit are recorded in the
   evidence directory.

## Usage assumptions

9. **Evaluation, research, and prototyping only.** AxiomRT v1.0 must not
   be used to control real vehicles, aircraft, medical devices, or any
   safety-critical machinery.
10. Security claims hold within the stated boundary (single hart, no
    user-visible DMA, supervisor configured as documented). Outside the
    boundary, no claim is made.
