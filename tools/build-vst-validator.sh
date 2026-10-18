#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
python3 tools/prepare-plugin-sdk.py --validator
cmake -S .tools/vst3-validation -B target/vst-validator -DCMAKE_BUILD_TYPE=Release -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF
cmake --build target/vst-validator --target validator --parallel 6
