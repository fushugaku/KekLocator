#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
mkdir -p target/KekLocatorNativeTest.app/Contents/MacOS
clang++ -std=c++17 -O2 -fobjc-arc tools/plugin-ui-host.mm -framework Cocoa -framework AudioUnit -framework AudioToolbox -o target/KekLocatorNativeTest.app/Contents/MacOS/KekLocatorUIHost
python3 - <<'PY'
import plistlib
from pathlib import Path
root=Path.cwd()
info={'CFBundleExecutable':'KekLocatorUIHost','CFBundleIdentifier':'local.ErikaRust.KekLocatorNativeUITest',
      'CFBundleName':'KekLocator Native Test','CFBundlePackageType':'APPL','CFBundleVersion':'1',
      'NSHighResolutionCapable':True,'PluginPath':str(root/'build/plugins/macos-universal/KekLocator.component'),
      'ReportPath':str(root/'research/plugin-ui-host.log')}
(root/'target/KekLocatorNativeTest.app/Contents/Info.plist').write_bytes(plistlib.dumps(info))
PY
codesign --force --sign - target/KekLocatorNativeTest.app
