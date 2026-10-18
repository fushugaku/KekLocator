#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
test "$(uname -s)" = Darwin
mkdir -p research
export RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTDOC="$(rustup which --toolchain stable rustdoc)"
rustup run stable cargo test --release --workspace > research/plugin-final-tests.log 2>&1
case "$(uname -m)" in arm64) task_target=aarch64-apple-darwin;; x86_64) task_target=x86_64-apple-darwin;; *) exit 1;; esac
task_library="target/$task_target/release/libkeklocator_plugin.a"
test -f "$task_library"
task_bundles="$task_root/build/plugins/macos-universal"
for task_ext in vst3 component clap; do
  codesign --verify --strict "$task_bundles/KekLocator.$task_ext"
  lipo -archs "$task_bundles/KekLocator.$task_ext/Contents/MacOS/KekLocator"
done
clang++ -std=c++17 -O2 -I .tools/iplug2/Dependencies/IPlug/CLAP_SDK/include tools/plugin-host-tests.mm "$task_library" -framework AudioToolbox -framework AudioUnit -framework CoreFoundation -framework Foundation -framework Security -framework SystemConfiguration -liconv -o target/plugin-host-tests > research/plugin-host-tests-build.log 2>&1
./target/plugin-host-tests "$task_bundles/KekLocator.component" "$task_bundles/KekLocator.clap/Contents/MacOS/KekLocator" > research/plugin-host-tests.log 2>&1
task_validator=target/vst-validator/bin/Release/validator
if [ ! -x "$task_validator" ]; then ./tools/build-vst-validator.sh; fi
clang++ -std=c++17 -O2 -DNDEBUG -I .tools/vst3-validation tools/plugin-vst3-host-tests.mm .tools/vst3-validation/public.sdk/source/common/memorystream.cpp target/vst-validator/lib/Release/libsdk_hosting.a target/vst-validator/lib/Release/libsdk_common.a target/vst-validator/lib/Release/libbase.a target/vst-validator/lib/Release/libpluginterfaces.a "$task_library" -framework Cocoa -framework Security -framework SystemConfiguration -liconv -o target/plugin-vst3-host-tests > research/plugin-vst3-host-tests-build.log 2>&1
./target/plugin-vst3-host-tests "$task_bundles/KekLocator.vst3" > research/plugin-vst3-host-tests.log 2>&1
"$task_validator" "$task_bundles/KekLocator.vst3" > research/plugin-vst3-validation.log 2>&1
if [ "${1:-}" = --original ]; then
  node tools/verify-original-wasm.mjs --daw > research/plugin-original-audio.log 2>&1
  node tools/verify-original-wasm.mjs research/original-reference/extended/controls --daw >> research/plugin-original-audio.log 2>&1
fi
cat research/plugin-host-tests.log research/plugin-vst3-host-tests.log
tail -n 3 research/plugin-vst3-validation.log
