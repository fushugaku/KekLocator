#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
python3 tools/prepare-firmware.py
export RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTDOC="$(rustup which --toolchain stable rustdoc)"
rustup target add --toolchain stable wasm32-unknown-unknown
task_bindgen="$task_root/.tools/bin/wasm-bindgen"
if [ ! -x "$task_bindgen" ]; then
  rustup run stable cargo install wasm-bindgen-cli --version 0.2.129 --locked --root "$task_root/.tools"
fi
mkdir -p dist/pkg/engine dist/pkg/ui
rustup run stable cargo build --release --target wasm32-unknown-unknown -p keklocator-engine --features wasm-api
"$task_bindgen" --target web --out-dir dist/pkg/engine --out-name engine target/wasm32-unknown-unknown/release/keklocator_engine.wasm
rustup run stable cargo build --release --target wasm32-unknown-unknown -p keklocator-ui
"$task_bindgen" --target web --out-dir dist/pkg/ui --out-name ui target/wasm32-unknown-unknown/release/keklocator_ui.wasm
cp web/* dist/
"$task_bindgen" --target no-modules --out-dir dist --out-name plugin-ui target/wasm32-unknown-unknown/release/keklocator_ui.wasm
python3 tools/package-plugin-ui.py
