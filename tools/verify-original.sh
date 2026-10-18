#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
task_python="$task_root/.tools/reference-python/bin/python"
if [ ! -x "$task_python" ]; then python3 -m venv "$task_root/.tools/reference-python"; fi
"$task_python" -m pip install -r tools/reference-requirements.txt
export RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTDOC="$(rustup which --toolchain stable rustdoc)"
rustup run stable cargo build --release -p keklocator-engine --examples
"$task_python" tools/audit_coverage.py
"$task_python" tools/verify_control_messages.py
"$task_python" tools/verify_original.py --controls --all "$@"
