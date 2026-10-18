KekLocator 1.2.0 — Windows x64

VST3 and CLAP audio effect. The audio engine is native Rust; the embedded editor
runs offline in Microsoft Edge WebView2. Install the Evergreen WebView2 Runtime
from https://developer.microsoft.com/microsoft-edge/webview2/ if it is missing.

INSTALL
Close your DAW, extract the whole ZIP, then run Install.ps1 in PowerShell:
  powershell -NoProfile -ExecutionPolicy Bypass -File .\Install.ps1
This installs for the current user without administrator rights:
  %LOCALAPPDATA%\Programs\Common\VST3\KekLocator.vst3
  %LOCALAPPDATA%\Programs\Common\CLAP\KekLocator.clap
Add these folders to the DAW plug-in search paths where necessary, then rescan.
You can also copy the VST3 bundle / CLAP file to your usual plug-in folders.
Keep the entire .vst3 directory together.

USE
Insert KekLocator as an audio effect. Original is the default sound mode.
To hear the internal sequencer, enable Source and Host, then press Play in the
DAW. Free uses its own BPM. Extended enables swing, probability, gate, ratchets,
effect locks, wet ducking, sound/pattern banks, A/B, and Undo/Redo.
The library is saved in %APPDATA%\KekLocator\library.json.

UPGRADE
KekLocator preserves the previous Echolocator Rust VST3/CLAP identities and state.
Keep only one version of each format installed to avoid duplicate registrations.
Uninstall by closing your DAW and removing the two plug-in files/directories.

Source, documentation, and releases: https://github.com/fushugaku/KekLocator
This is an unofficial software port. See NOTICE.md for source attribution.
