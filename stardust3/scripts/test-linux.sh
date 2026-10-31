#!/usr/bin/env bash
set -euo pipefail
if [[ $(uname -s) != Linux ]]; then
    printf 'Run this script on native Linux or inside Ubuntu WSL.\n' >&2
    exit 2
fi
cargo_options=(--locked)
case "${1:-}" in
    '') ;;
    --offline) cargo_options+=(--offline) ;;
    *) printf 'Usage: bash scripts/test-linux.sh [--offline]\n' >&2; exit 2 ;;
esac
if [[ $# -gt 1 ]]; then exit 2; fi
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
for tool in cargo rustc cc timeout; do
    if ! command -v "$tool" >/dev/null; then
        printf 'Missing Linux build tool: %s. Install Rust (see rust-toolchain.toml) and a C compiler.\n' "$tool" >&2
        exit 1
    fi
done
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
report_dir="$project_root/target/validation"
mkdir -p -- "$report_dir"
exec > >(tee "$report_dir/linux.log") 2>&1

# Permission, symlink, FIFO and rename tests must run on a Linux filesystem,
# even when the source project is on a Windows drive mounted by WSL.
validation_dir=$(mktemp -d /tmp/stardust-validation.XXXXXXXX)
cleanup() {
    local status=$?
    local resolved
    resolved=$(realpath -e -- "$validation_dir") || return "$status"
    if [[ $resolved == "$validation_dir" && $resolved == /tmp/stardust-validation.* ]]; then
        rm -rf -- "$resolved"
    fi
    if [[ $status -ne 0 ]]; then printf 'Linux validation FAILED (exit %s).\n' "$status"; fi
    return "$status"
}
trap cleanup EXIT
mkdir -p -- "$validation_dir/project" "$validation_dir/tmp"
cp -- "$project_root/Cargo.toml" "$project_root/Cargo.lock" "$project_root/rust-toolchain.toml" "$validation_dir/project/"
cp -R -- "$project_root/src" "$project_root/tests" "$validation_dir/project/"
export TMPDIR="$validation_dir/tmp"
export CARGO_TARGET_DIR="$project_root/target/linux"
export RUST_BACKTRACE=1
cd -- "$validation_dir/project"
printf 'Linux validation started: '
date -u '+%Y-%m-%dT%H:%M:%SZ'
uname -srmo
rustc --version --verbose
if [[ $(rustc --version --verbose) != *-linux-* ]]; then
    printf 'A native Linux Rust toolchain is required.\n' >&2
    exit 1
fi
sha256sum Cargo.toml Cargo.lock
cargo fmt --all -- --check
timeout --kill-after=10s 15m cargo test --release "${cargo_options[@]}" -- --test-threads "${STARDUST_TEST_THREADS:-2}"
timeout --kill-after=10s 15m cargo clippy --all-targets "${cargo_options[@]}" -- -D warnings
timeout --kill-after=10s 15m cargo build --release "${cargo_options[@]}"
printf 'Linux validation PASSED.\n'
