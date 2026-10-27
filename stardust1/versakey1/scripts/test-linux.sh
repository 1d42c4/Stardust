#!/usr/bin/env bash
set -euo pipefail

# WSL's non-interactive shells may not load rustup's PATH setup.
if ! command -v cargo >/dev/null 2>&1; then
    cargo_environment="${CARGO_HOME:-$HOME/.cargo}/env"
    if [[ -f "$cargo_environment" ]]; then
        source "$cargo_environment"
    fi
fi

project_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$project_directory"

# Keep Linux and Windows build artifacts separate when using WSL.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$project_directory/target/linux}"

cargo fmt --all -- --check
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
