#!/usr/bin/env sh
set -eu
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
cargo update -p wasm-bindgen
