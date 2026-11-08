#!/bin/sh
#
# CI script for the `coin selection` crate.

set -ex

# FEATURES="rand"

# Sanity checks.
cargo --version
rustc --version

cargo build --all-features
cargo test --all-features

if [ "$DO_FEATURE_MATRIX" = true ]; then
    # No features.
    cargo build --no-default-features
    cargo test --no-default-features
fi

if [ "$DO_LINT" = true ]
then
    rustup component add clippy
    cargo clippy --all-features --all-targets -- -D warnings
fi
