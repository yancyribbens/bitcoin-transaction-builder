#!/bin/sh
#
# CI script for testing the MSRV of the `transaction builder`.

# -e (errexit) will immediately exit on script fail.
# -x (xtrace) enable execution tracing with expanded variables and arguments.
set -ex

# Sanity checks.
cargo --version
rustc --version

# set the Cargo.toml msrv dependencies
cp Cargo.msrv.toml Cargo.toml
cp Cargo.msrv.lock Cargo.lock

cargo build --all-features
# arbtest not available for cargo test --all-features

if [ "$DO_FEATURE_MATRIX" = true ]; then
    # No features.
    cargo build --no-default-features
    cargo test --no-default-features
fi
