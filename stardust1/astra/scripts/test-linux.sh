#!/usr/bin/env bash
# Run on native Linux or in WSL. --install-tools installs missing Ubuntu build
# tools and the two Rust toolchains; sudo, if needed, prompts in your terminal.
set -euo pipefail

install_tools=false
case "${1:-}" in
    "") ;;
    --install-tools) install_tools=true ;;
    *) printf 'Usage: bash scripts/test-linux.sh [--install-tools]\n' >&2; exit 2 ;;
esac
if [[ $# -gt 1 || $(uname -s) != Linux ]]; then
    printf 'Run this script inside Linux or Ubuntu WSL.\n' >&2
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
project_root=$(cd -- "$script_dir/.." && pwd -P)
report_dir="$project_root/target/validation"
mkdir -p -- "$report_dir"
exec > >(tee "$report_dir/linux.log") 2>&1

# Keep builds and permission/symlink test fixtures on Linux's own filesystem.
# Windows and Linux must never share Cargo's build output directory.
validation_dir=$(mktemp -d /tmp/astra-xor-validation.XXXXXXXX)
cleanup() {
    local status=$?
    if [[ $status -ne 0 ]]; then
        printf '\nLinux validation FAILED (exit %s). Log: %s/linux.log\n' "$status" "$report_dir"
    fi
    local resolved
    resolved=$(realpath -e -- "$validation_dir") || return "$status"
    if [[ $resolved == "$validation_dir" && $resolved == /tmp/astra-xor-validation.* ]]; then
        rm -rf -- "$resolved"
    fi
    return "$status"
}
trap cleanup EXIT

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if "$install_tools"; then
    if ! command -v cc >/dev/null || ! command -v curl >/dev/null || ! command -v timeout >/dev/null; then
        if ! command -v apt-get >/dev/null; then
            printf 'Install a C linker, curl and GNU coreutils for your Linux distribution.\n' >&2
            exit 1
        fi
        sudo apt-get update
        sudo apt-get install -y build-essential curl ca-certificates coreutils
    fi
    if ! command -v rustup >/dev/null; then
        curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
            https://sh.rustup.rs -o "$validation_dir/rustup-init.sh"
        sh "$validation_dir/rustup-init.sh" -y --profile minimal \
            --default-toolchain stable --no-modify-path
    fi
    for toolchain in stable 1.88.0; do
        rustup toolchain install "$toolchain" --profile minimal --component rustfmt --component clippy
    done
fi

for tool in cargo rustup cc timeout sha256sum; do
    if ! command -v "$tool" >/dev/null; then
        printf 'Missing %s. On Ubuntu, rerun with --install-tools.\n' "$tool" >&2
        exit 1
    fi
done

# Snapshot the source, so edits in the Windows workspace cannot change a run.
mkdir -p -- "$validation_dir/project/src" "$validation_dir/tmp"
cp -- "$project_root/Cargo.toml" "$project_root/Cargo.lock" "$validation_dir/project/"
cp -R -- "$project_root/src/." "$validation_dir/project/src/"
cp -R -- "$project_root/tests" "$validation_dir/project/tests"
cd -- "$validation_dir/project"
export CARGO_TARGET_DIR="$validation_dir/target"
export TMPDIR="$validation_dir/tmp"
export RUST_BACKTRACE=1
umask 022
printf 'Linux validation started: '
date -u '+%Y-%m-%dT%H:%M:%SZ'
uname -srmo
sha256sum Cargo.toml Cargo.lock
find src tests -type f -name '*.rs' -print0 | sort -z | xargs -0 sha256sum

for toolchain in stable 1.88.0; do
    printf '\nValidating Rust %s\n' "$toolchain"
    rustup run "$toolchain" rustc --version --verbose
    if [[ $(rustup run "$toolchain" rustc --version --verbose) != *-linux-* ]]; then
        printf 'A native Linux Rust toolchain is required.\n' >&2
        exit 1
    fi
    if [[ $toolchain == stable ]]; then
        cargo +"$toolchain" fmt --check
    fi
    timeout --kill-after=10s 5m cargo +"$toolchain" clippy --offline --locked --all-targets -- -D warnings
    timeout --kill-after=10s 5m cargo +"$toolchain" test --offline --locked
    timeout --kill-after=10s 5m cargo +"$toolchain" test --release --offline --locked
    timeout --kill-after=10s 5m cargo +"$toolchain" build --release --offline --locked
done
printf '\nLinux validation PASSED on stable and Rust 1.88.0. Log: %s/linux.log\n' "$report_dir"
