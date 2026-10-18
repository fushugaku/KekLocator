#!/usr/bin/env python3
import base64
import re
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
source=ROOT/'dist';dest=ROOT/'plugins/resources/web';dest.mkdir(parents=True,exist_ok=True)
(source/'plugin-ui-bytes.js').write_text('globalThis.keklocatorUiBytes="'+base64.b64encode((source/'plugin-ui_bg.wasm').read_bytes()).decode()+'";\n')
for name in ['plugin.html','plugin-style.css','plugin-ui.js','plugin-ui-bytes.js','plugin-bridge.js']:
 (dest/('index.html'if name=='plugin.html'else name)).write_bytes((source/name).read_bytes())

# WebView2 receives one self-contained HTML resource from the native DLL.
# NavigateToString has a 2 MiB limit; keep all generated assets below it.
html=(source/'plugin.html').read_text(encoding='utf-8')
html=html.replace('<link rel="stylesheet" href="./plugin-style.css">', '<style>'+(source/'plugin-style.css').read_text(encoding='utf-8')+'</style>')
for name in ['plugin-ui.js','plugin-ui-bytes.js','plugin-bridge.js']:
 script=(source/name).read_text(encoding='utf-8')
 script=re.sub(r'</script',r'<\\/script',script,flags=re.I)
 html=html.replace('<script src="./'+name+'"></script>','<script>'+script+'</script>')
if '<script src=' in html or '<link rel="stylesheet"' in html:
 raise RuntimeError('The Windows editor still references external assets')
if len(html.encode('utf-8'))>=2*1024*1024:
 raise RuntimeError('The embedded Windows editor exceeds the WebView2 HTML limit')
(source/'plugin-native.html').write_text(html,encoding='utf-8')
print('Packaged self-contained Windows editor:',len(html.encode('utf-8')),'bytes')
