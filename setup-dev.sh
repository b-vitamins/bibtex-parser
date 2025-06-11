#!/bin/bash
set -euo pipefail

# setup-dev.sh - Provision development environment for bibtex-parser
# This script installs toolchains and prefetches dependencies so that the
# repository can be built and tested offline.
# It should be run with network access.

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "[1/6] Installing system packages via apt..."
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config git curl \
    python3 python3-venv python3-pip \
    valgrind linux-tools-common linux-tools-generic

# Install rustup if needed
if ! command -v rustup >/dev/null; then
    echo "[2/6] Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.75.0
    source "$HOME/.cargo/env"
else
    source "$HOME/.cargo/env"
    rustup toolchain install 1.75.0
fi

# Use the toolchain specified in Cargo.toml
rustup default 1.75.0
rustup component add rustfmt clippy

# Install additional cargo tools if not present
if ! command -v cargo-tarpaulin >/dev/null; then
    echo "[3/6] Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi
if ! command -v flamegraph >/dev/null; then
    echo "[4/6] Installing flamegraph..."
    cargo install flamegraph
fi

# Set up Python virtual environment
if [ ! -d "$REPO_ROOT/.venv" ]; then
    echo "[5/6] Creating Python virtual environment..."
    python3 -m venv "$REPO_ROOT/.venv"
fi
source "$REPO_ROOT/.venv/bin/activate"
pip install --upgrade pip
pip install rich
deactivate

# Prefetch Rust dependencies
echo "[6/6] Fetching Rust crates..."
cd "$REPO_ROOT"
cargo fetch

# Build and test once to populate target directory
cargo build --all-targets
cargo test --all-features

echo "\nDevelopment environment setup complete. You can now work offline."
