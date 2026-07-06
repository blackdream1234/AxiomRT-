#!/bin/sh
# AxiomRT one-command setup (AXIOM-INSTALL-001).
# Requirement reference: AxiomrtFull Completion Mode.md §13,
# docs/20_REAL_OS_PRODUCT_DEFINITION.md §6, docs/09_BUILD_AND_BOOT.md.
#
# Checks (and, with consent, installs) the toolchain, adds the RISC-V
# target, builds the kernel, and runs the boot smoke test.
#
# Usage: ./install.sh [-y]
#   -y   assume yes: run package installs without prompting (needs sudo)
#
# The script never touches credentials, never installs anything without
# telling you the exact command, and is safe to re-run.

set -u

ASSUME_YES=0
[ "${1:-}" = "-y" ] && ASSUME_YES=1

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO_ROOT"

say()  { printf '%s\n' "$*"; }
ok()   { printf 'OK       %s\n' "$*"; }
miss() { printf 'MISSING  %s\n' "$*"; }

fail=0

# --- 1. Detect Linux distribution -----------------------------------
DISTRO=unknown
PKG=""
if [ -r /etc/os-release ]; then
    . /etc/os-release
    DISTRO="${ID:-unknown}"
fi
case "$DISTRO" in
    debian|ubuntu|kali|linuxmint|pop) PKG="apt" ;;
    fedora|rhel|centos|rocky|alma)    PKG="dnf" ;;
    arch|manjaro|endeavouros)         PKG="pacman" ;;
    opensuse*|sles)                   PKG="zypper" ;;
esac
say "AxiomRT installer — detected distribution: $DISTRO (package manager: ${PKG:-unknown})"
say ""

# Package names per manager: qemu riscv64 system emulator, coq.
pkg_install_cmd() {
    case "$PKG" in
        apt)    echo "sudo apt-get install -y $1" ;;
        dnf)    echo "sudo dnf install -y $1" ;;
        pacman) echo "sudo pacman -S --noconfirm $1" ;;
        zypper) echo "sudo zypper install -y $1" ;;
        *)      echo "" ;;
    esac
}
qemu_pkg() {
    case "$PKG" in
        apt)    echo "qemu-system-misc" ;;
        dnf)    echo "qemu-system-riscv" ;;
        pacman) echo "qemu-system-riscv" ;;
        zypper) echo "qemu" ;;
        *)      echo "" ;;
    esac
}

# try_install <human name> <package>
# Prints the exact command; runs it only with consent.
try_install() {
    _name="$1"; _pkg="$2"
    _cmd="$(pkg_install_cmd "$_pkg")"
    if [ -z "$_cmd" ]; then
        say "         install $_name manually (unknown package manager)"
        return 1
    fi
    say "         install command: $_cmd"
    if [ "$ASSUME_YES" -eq 1 ]; then
        $_cmd
        return $?
    fi
    printf '         run it now? [y/N] '
    read -r answer
    case "$answer" in
        y|Y) $_cmd ;;
        *)   say "         skipped — run it yourself, then re-run ./install.sh"; return 1 ;;
    esac
}

# --- 2./3. Rust and cargo --------------------------------------------
if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
    ok "rust: $(rustc --version)"
else
    miss "rust/cargo"
    say "         install via rustup (recommended, no sudo):"
    say "         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    say "         then: source \"\$HOME/.cargo/env\" and re-run ./install.sh"
    fail=1
fi

# --- 4. QEMU RISC-V ---------------------------------------------------
if command -v qemu-system-riscv64 >/dev/null 2>&1; then
    ok "qemu: $(qemu-system-riscv64 --version | head -n1)"
else
    miss "qemu-system-riscv64"
    if ! try_install "QEMU RISC-V" "$(qemu_pkg)"; then fail=1; fi
    command -v qemu-system-riscv64 >/dev/null 2>&1 || fail=1
fi

# --- 5. Coq (optional: proof recompilation only) ---------------------
if command -v coqc >/dev/null 2>&1; then
    ok "coq: $(coqc --version | head -n1)"
else
    say "absent   coq — optional; only needed to recompile proofs (package: coq)"
fi

# --- 7. RISC-V target -------------------------------------------------
if command -v rustup >/dev/null 2>&1; then
    if rustup target list --installed | grep -qx riscv64gc-unknown-none-elf; then
        ok "rust target riscv64gc-unknown-none-elf"
    else
        say "adding rust target riscv64gc-unknown-none-elf"
        rustup target add riscv64gc-unknown-none-elf || fail=1
    fi
elif command -v rustc >/dev/null 2>&1; then
    say "unknown  riscv64gc target (no rustup; assuming a vendored toolchain provides it)"
fi

if [ "$fail" -ne 0 ]; then
    say ""
    say "install: FAIL — fix the MISSING items above and re-run ./install.sh"
    exit 1
fi

# --- 8. Build ---------------------------------------------------------
say ""
say "building AxiomRT (release, riscv64gc bare metal)"
if ! cargo build --release; then
    say "install: FAIL — kernel build failed"
    exit 1
fi

# --- 9. Boot smoke test ----------------------------------------------
say ""
say "running boot smoke test (QEMU)"
if ! ./tests/boot_smoke_test.sh; then
    say "install: FAIL — boot smoke test failed"
    exit 1
fi

# --- 10. Next commands ------------------------------------------------
say ""
say "install: PASS"
say ""
say "Next commands:"
say "  cargo axiomctl doctor      # re-check the environment any time"
say "  cargo axiomctl demo full   # full fault-containment demo (exit QEMU: Ctrl-A x)"
say "  cargo axiomctl verify      # full verification sweep (QEMU + host + Coq)"
say "  cargo studio               # local dashboard at http://127.0.0.1:8787/"
say ""
say "Read kit/LIMITATIONS.md and kit/ASSUMPTIONS_OF_USE.md before drawing"
say "conclusions — evaluation-stage software, no certification claim."
exit 0
