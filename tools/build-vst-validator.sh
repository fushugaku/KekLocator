#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$task_root"
python3 - <<'PY'
import json,subprocess
from pathlib import Path
root=Path.cwd();sdk=root/'.tools/vst3-validation'
source=json.loads((root/'plugins/SOURCES.json').read_text())['VST3_SDK']
if not (sdk/'CMakeLists.txt').is_file():
    sdk.mkdir(parents=True,exist_ok=True)
    subprocess.run(['git','init',str(sdk)],check=True)
    subprocess.run(['git','-C',str(sdk),'fetch','--depth','1',source['url'],source['commit']],check=True)
    subprocess.run(['git','-C',str(sdk),'checkout','--detach','FETCH_HEAD'],check=True)
    subprocess.run(['git','-C',str(sdk),'submodule','update','--init','--depth','1',*source['submodules']],check=True)
# Only the CMake generator check changes. Validator/test sources stay intact.
path=sdk/'cmake/modules/SMTG_Global.cmake'
text=path.read_text();call='\nsmtg_detect_xcode_version()\n'
guard='\nif(XCODE)\n    smtg_detect_xcode_version()\nendif()\n'
if call in text:path.write_text(text.replace(call,guard,1))
elif guard not in text:raise SystemExit('Unexpected SDK CMake setup; review Xcode detection first')
PY
cmake -S .tools/vst3-validation -B target/vst-validator -DCMAKE_BUILD_TYPE=Release -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF
cmake --build target/vst-validator --target validator --parallel 6
