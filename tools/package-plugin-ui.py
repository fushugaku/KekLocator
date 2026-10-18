#!/usr/bin/env python3
import base64
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
source=ROOT/'dist';dest=ROOT/'plugins/resources/web';dest.mkdir(parents=True,exist_ok=True)
(source/'plugin-ui-bytes.js').write_text('globalThis.keklocatorUiBytes="'+base64.b64encode((source/'plugin-ui_bg.wasm').read_bytes()).decode()+'";\n')
for name in ['plugin.html','plugin-style.css','plugin-ui.js','plugin-ui-bytes.js','plugin-bridge.js']:
 (dest/('index.html'if name=='plugin.html'else name)).write_bytes((source/name).read_bytes())
