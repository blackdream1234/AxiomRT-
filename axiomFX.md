You are working on AxiomRT.

Current verified state:
AxiomRT has reached v1.2-app-loader. QEMU boots to an interactive `axiom>` shell. The shell supports app commands through a U-mode app_loader_service. Static applications `hello`, `counter`, and `fault_demo` run from the shell. `fault_demo` is contained and the shell remains alive. `./scripts/verify_all.sh` passes locally with 11/11 QEMU tests, all host suites, axiomctl/studio tests, 3 Coq model files, zero warnings, and clippy -D warnings clean.

Next mandatory phase:
AXIOM-FS — Read-only Filesystem Service

Goal:
Add a minimal read-only filesystem service as a user-space service. The shell must support `ls` and `cat` by sending bounded IPC requests to `fs_service`. The kernel must not contain filesystem logic.

Important boundaries:

* Do not implement storage yet.
* Do not implement block devices yet.
* Do not implement writable filesystem yet.
* Do not implement dynamic app loading from filesystem yet.
* Do not add networking.
* Do not add real hardware BSP.
* Do not put filesystem policy or path parsing inside the kernel.

Architecture rule:
The kernel provides only mechanisms:

* user-space tasks,
* address spaces,
* IPC,
* capabilities,
* fault containment,
* scheduling.

Filesystem policy lives in:

* fs_service,
* shell_service,
* future storage_service.

Required tasks:

AXIOM-FS-001:
Create `docs/28_READONLY_FILESYSTEM_SERVICE.md`.

Document:

1. Why filesystem is user-space.
2. Difference between embedded read-only filesystem and future storage-backed filesystem.
3. IPC protocol.
4. Path syntax.
5. File table.
6. Shell commands.
7. Security rules.
8. Limitations.
9. Future storage transition.

AXIOM-FS-002:
Define read-only filesystem image.

Initial files:

* `/etc/version`
* `/etc/limitations`
* `/apps/hello.manifest`
* `/apps/counter.manifest`
* `/apps/fault_demo.manifest`
* `/docs/about`

Each file must be static, bounded, read-only, and stored in the user-accessible service region or service-owned static region according to the existing constrained-Rust rules.

AXIOM-FS-003:
Create `fs_service` as a constrained U-mode service.

Responsibilities:

* receive filesystem requests over bounded IPC,
* parse simple paths,
* handle `ls`,
* handle `cat`,
* return bounded response chunks,
* never crash on malformed input,
* never access kernel-only memory.

AXIOM-FS-004:
Define filesystem IPC protocol.

Request examples:

* `LS /`
* `LS /etc`
* `LS /apps`
* `CAT /etc/version`
* `CAT /docs/about`

Response examples:

* `OK file1 file2 file3`
* `OK <content>`
* `ERR not_found`
* `ERR too_long`
* `ERR bad_path`

AXIOM-FS-005:
Add shell commands:

* `ls`
* `ls <path>`
* `cat <path>`

Preserve all existing shell commands:

* help
* version
* tasks
* faults
* ipc
* caps
* memory
* uptime
* events
* apps
* app info <name>
* run <name>
* run demo
* kill <idx>
* restart <idx>
* clear
* shutdown

AXIOM-FS-006:
Capability model.

Add filesystem capabilities:

* fs_read
* fs_list

Only shell_service receives fs_read/fs_list.
Apps do not receive filesystem capability yet unless explicitly granted later.
fault_demo must not gain filesystem access.

AXIOM-FS-007:
Add QEMU test:
`tests/readonly_fs_qemu_test.sh`

The test must assert:

* boot reaches `axiom>`
* `ls` lists at least `etc apps docs`
* `ls /etc` lists `version limitations`
* `cat /etc/version` prints AxiomRT version/stage
* `cat /apps/hello.manifest` prints hello manifest
* invalid path returns `ERR not_found`
* overlong path is rejected or truncated safely
* shell remains alive after invalid requests
* `shutdown` performs controlled QEMU exit 0

AXIOM-FS-008:
Integrate into `./scripts/verify_all.sh`.

After this phase, verify_all must report:

* 12/12 QEMU tests
* host tests pass
* axiomctl tests pass
* studio tests pass
* Coq files compile
* zero warnings
* clippy -D warnings clean

AXIOM-FS-009:
Archive evidence under `evidence/v1.3`.

Evidence must include:

* readonly_fs_qemu_test.log
* verify_all.log
* tool_versions.txt
* REPORT.md

Expected final tag:
`v1.3-readonly-fs`

Allowed files:

* docs/28_READONLY_FILESYSTEM_SERVICE.md
* kernel/src/user/*
* kernel/src/services/*
* kernel/src/syscall/*
* kernel/src/arch/riscv64/*
* kernel/src/monitor/*
* kernel/src/main.rs
* kernel/linker.ld
* tests/readonly_fs_qemu_test.sh
* scripts/verify_all.sh
* userland/*
* tools/axiomctl/*
* studio/*
* evidence/v1.3/*
* README.md only for milestone/status update

Forbidden:

* storage/block-device implementation
* writable filesystem
* external ELF/app loading
* network implementation
* hardware BSP
* certification claims
* weakening existing tests
* removing existing shell/app commands
* hiding limitations

Commands required:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
```

Definition of done:

* `axiom>` shell still works.
* `ls` works.
* `ls /etc` works.
* `ls /apps` works.
* `cat /etc/version` works.
* `cat /apps/hello.manifest` works.
* invalid paths fail safely.
* shell remains alive after filesystem errors.
* filesystem logic is in fs_service/user-space, not kernel.
* `tests/readonly_fs_qemu_test.sh` passes.
* `./scripts/verify_all.sh` ends with `VERIFY ALL: PASS`.
* no warnings.
* clippy -D warnings clean.
* no certification or production claim.
* evidence archived under `evidence/v1.3`.
* final tag `v1.3-readonly-fs`.

Commit sequence:

* `AXIOM-FS-001: document read-only filesystem service`
* `AXIOM-FS-002: add static read-only filesystem image`
* `AXIOM-FS-003: add fs service`
* `AXIOM-FS-004: add filesystem IPC protocol`
* `AXIOM-FS-005: add shell filesystem commands`
* `AXIOM-FS-006: enforce filesystem capabilities`
* `AXIOM-FS-007: add read-only filesystem QEMU test`
* `AXIOM-FS-008: integrate filesystem test into verification sweep`
* `AXIOM-FS-009: archive v1.3 read-only filesystem evidence`
