#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
export RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTDOC="$(rustup which --toolchain stable rustdoc)"
rustup run stable cargo test --release --workspace
rustup run stable cargo build --release -p keklocator
./target/release/keklocator verify
node tools/verify-wasm.mjs
