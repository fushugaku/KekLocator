#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
task_ghidra="${ERIKA_GHIDRA_HOME:?Set ERIKA_GHIDRA_HOME to the Ghidra installation directory}"
task_project="${ERIKA_REVERSE_PROJECT:-$task_root/../echolocator-1.05-reverse/ghidra}"
mkdir -p research
"$task_ghidra/support/analyzeHeadless" "$task_project" Echolocator-1.05 \
  -process echolocator-1.05.bin -readOnly -scriptPath "$task_root/tools" \
  -preScript SeedCallbacks.java -postScript ExportPcode.java "$task_root/research/pcode.json"
python3 tools/lift_rust.py
python3 tools/export_parameters.py
