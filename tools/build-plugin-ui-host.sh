#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
mkdir -p target/KekLocatorUIHost.app/Contents/MacOS
clang++ -std=c++17 -O2 -fobjc-arc tools/plugin-ui-host.mm -framework Cocoa -framework AudioUnit -framework AudioToolbox -o target/KekLocatorUIHost.app/Contents/MacOS/KekLocatorUIHost
python3 - <<'PY'
import plistlib
from pathlib import Path
root=Path.cwd()
info={'CFBundleExecutable':'KekLocatorUIHost','CFBundleIdentifier':'local.ErikaRust.EditorTest',
      'CFBundleName':'KekLocator UI Host','CFBundlePackageType':'APPL','CFBundleVersion':'1',
      'NSHighResolutionCapable':True,'PluginPath':str(root/'build/plugins/macos-universal/KekLocator.component'),
      'ReportPath':str(root/'research/plugin-ui-host.log')}
(root/'target/KekLocatorUIHost.app/Contents/Info.plist').write_bytes(plistlib.dumps(info))
PY
codesign --force --sign - target/KekLocatorUIHost.app
