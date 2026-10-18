#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
python3 tools/prepare-firmware.py
python3 tools/prepare-plugin-sdk.py
export RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTDOC="$(rustup which --toolchain stable rustdoc)"
./tools/build-web.sh
case "$(uname -s)" in
  Darwin)
    rustup target add --toolchain stable aarch64-apple-darwin x86_64-apple-darwin
    rustup run stable cargo build --release --target aarch64-apple-darwin -p keklocator-plugin
    rustup run stable cargo build --release --target x86_64-apple-darwin -p keklocator-plugin
    mkdir -p target/universal
    lipo -create target/aarch64-apple-darwin/release/libkeklocator_plugin.a target/x86_64-apple-darwin/release/libkeklocator_plugin.a -output target/universal/libkeklocator_plugin.a
    cmake -S plugins -B target/plugin-build-universal -DCMAKE_BUILD_TYPE=Release -DIPLUG2_UNIVERSAL=ON -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 -DECHO_RUST_LIBRARY="$task_root/target/universal/libkeklocator_plugin.a"
    cmake --build target/plugin-build-universal --parallel 6
    mkdir -p build/plugins/macos-universal
    for task_suffix in vst3 component clap; do
      task_bundle="KekLocator.$task_suffix"
      ditto "target/plugin-build-universal/out/$task_bundle" "build/plugins/macos-universal/$task_bundle"
      codesign --force --sign - "build/plugins/macos-universal/$task_bundle"
      codesign --verify --strict "build/plugins/macos-universal/$task_bundle"
    done
    ditto -c -k --keepParent build/plugins/macos-universal build/plugins/KekLocator-macOS-universal.zip
    ;;
  *) printf '%s\n' 'For Windows use tools/build-plugins.ps1.'; exit 1;;
esac
printf '%s\n' 'Built: build/plugins/macos-universal (VST3, AU, CLAP)'
