#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$task_root"
./tools/build-web.sh
RUSTC="$(rustup which --toolchain stable rustc)" rustup run stable cargo run --release -p keklocator -- serve "$@"
