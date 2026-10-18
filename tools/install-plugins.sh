#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
task_source="$task_root/build/plugins/macos-universal"
test -d "$task_source/KekLocator.component"
for task_pair in 'vst3:VST3' 'component:Components' 'clap:CLAP'; do
  task_extension=${task_pair%:*}
  task_folder=${task_pair#*:}
  task_dest="$HOME/Library/Audio/Plug-Ins/$task_folder/KekLocator.$task_extension"
  if [ -e "$task_dest" ]; then
    task_id=$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$task_dest/Contents/Info.plist")
    case "$task_id" in local.ErikaRust.*) ;; *) printf 'Different plug-in already exists: %s\n' "$task_dest"; exit 1;;esac
  fi
  mkdir -p "$HOME/Library/Audio/Plug-Ins/$task_folder"
  ditto "$task_source/KekLocator.$task_extension" "$task_dest"
done
printf '%s\n' 'Installed KekLocator VST3 / AU / CLAP into ~/Library/Audio/Plug-Ins/'
