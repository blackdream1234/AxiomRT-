You are working on AxiomRT.

Current verified state:
AxiomRT reached v1.1 OS Shell Milestone. QEMU boots through OpenSBI → kernel → Sv39 MMU → timer → init_service → supervisor/logger/console/shell services → interactive `axiom>` prompt. `./scripts/verify_all.sh` passes locally with 10/10 QEMU tests, host suites, 3 Coq files, zero warnings, and clippy -D warnings clean. The project has CLI, event parser, Studio dashboard, install script, and CI workflow files.

Do not regress any existing behavior.

The next mandatory phase is:

AXIOM-APP — Application Model + Loader

Goal:
Move from built-in demos and embedded services toward a real OS application model. The shell must be able to list and start user applications by name, initially from a static built-in application manifest, without adding filesystem or external ELF loading yet.

Important boundary:
This is not filesystem work.
This is not storage work.
This is not networking.
This is not real hardware.
This is not certification.
Do not add dynamic loading from disk yet.
Do not put app policy in the kernel.

Architecture rule:
The kernel provides only mechanisms:

* create address space from a static app image,
* map app code/rodata/stack,
* assign capabilities from a manifest,
* start/kill/restart tasks,
* contain app faults.

User-space policy lives in:

* app_loader_service,
* shell_service,
* init_service.

Required tasks:

AXIOM-APP-001:
Create `docs/27_APPLICATION_MODEL.md`.

Document:

1. Difference between services and applications.
2. Current static app-table stage.
3. Future ELF/restricted image loader stage.
4. Application manifest fields.
5. Capability grants per app.
6. App lifecycle: Available → Loaded → Running → Exited/Faulted/Killed.
7. Shell commands.
8. Security rules.
9. Limitations.

AXIOM-APP-002:
Add a static application manifest.

Applications:

* hello
* fault_demo
* counter

Manifest fields:

* name
* entry point
* priority
* stack size/page
* allowed capabilities
* restart policy
* description

AXIOM-APP-003:
Create app_loader_service as a constrained U-mode service.

Responsibilities:

* own app policy,
* receive shell request over bounded IPC,
* validate app name,
* request kernel start by app index,
* return result to shell.

AXIOM-APP-004:
Add shell commands:

* apps
* app info <name>
* run <name>
* kill <idx>
* restart <idx>

Preserve existing:

* help
* tasks
* uptime
* events
* run demo
* shutdown

AXIOM-APP-005:
Implement hello app.

Behavior:

* starts in its own address space,
* prints “hello from app: hello” through console/log path,
* exits cleanly.

AXIOM-APP-006:
Implement fault_demo app.

Behavior:

* attempts an unauthorized operation or watchdog-triggering loop,
* kernel contains it,
* supervisor receives fault,
* shell remains alive.

AXIOM-APP-007:
Implement counter app.

Behavior:

* yields periodically,
* emits several progress events,
* exits cleanly.

AXIOM-APP-008:
Add QEMU test:
`tests/app_loader_qemu_test.sh`

The test must assert:

* boot reaches `axiom>`
* `apps` lists hello, fault_demo, counter
* `app info hello` works
* `run hello` starts hello app and it exits cleanly
* `run fault_demo` is contained and shell remains alive
* `run counter` runs and exits
* `shutdown` performs controlled QEMU exit 0

AXIOM-APP-009:
Integrate into `./scripts/verify_all.sh`.

After this phase, verify_all must report:

* 11/11 QEMU tests
* host tests pass
* axiomctl tests pass
* studio tests pass
* Coq files compile
* zero warnings
* clippy -D warnings clean

Allowed files:

* docs/27_APPLICATION_MODEL.md
* kernel/src/user/*
* kernel/src/services/*
* kernel/src/syscall/*
* kernel/src/dispatch/*
* kernel/src/monitor/*
* kernel/src/main.rs
* kernel/linker.ld
* tests/app_loader_qemu_test.sh
* scripts/verify_all.sh
* userland/*
* tools/axiomctl/*
* studio/*
* evidence/v1.2/*
* README.md only if status/phase is updated

Forbidden:

* filesystem implementation
* storage implementation
* network implementation
* hardware BSP
* certification claims
* weakening existing tests
* removing existing shell commands
* removing existing demos
* hiding limitations

Commands required:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
```

Definition of done:

* `axiom>` shell still works.
* `apps` lists static apps.
* `run hello` works.
* `run fault_demo` is contained.
* `run counter` works.
* shell remains alive after app fault.
* `tests/app_loader_qemu_test.sh` passes.
* `./scripts/verify_all.sh` ends with `VERIFY ALL: PASS`.
* No warnings.
* No certification or production claim.
* Evidence archived under `evidence/v1.2`.
* Commit one task at a time.
* Final tag:
  `v1.2-app-loader`.

Commit sequence:

* `AXIOM-APP-001: document application model`
* `AXIOM-APP-002: add static application manifest`
* `AXIOM-APP-003: add app loader service`
* `AXIOM-APP-004: add shell application commands`
* `AXIOM-APP-005: add hello application`
* `AXIOM-APP-006: add fault demo application`
* `AXIOM-APP-007: add counter application`
* `AXIOM-APP-008: add app loader QEMU test`
* `AXIOM-APP-009: archive v1.2 app loader evidence`
