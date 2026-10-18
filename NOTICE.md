# Source attribution

KekLocator is an unofficial software port of the digital audio processing in
**Erica Synths Echolocator firmware 1.05**. Erica Synths and Echolocator are the
names of the original manufacturer and product. This project is not affiliated
with or endorsed by Erica Synths.

The firmware-derived static Rust translation is in `crates/engine/src/generated`.
The original firmware and factory bank remain manufacturer material; renaming
this software does not change their ownership or grant a license to that material.
The repository does not contain the original UF2, extracted ROM or factory-bank
binary. `tools/prepare-firmware.py` downloads the pinned originals from the
manufacturer and verifies their hashes. Provenance is recorded in `PORTING.md`
and `crates/engine/firmware/source.json`.

This repository does not assert a blanket open-source license over the
firmware-derived code or factory data.

## Build dependencies

Native format and WebView glue uses iPlug2, the Steinberg VST3 SDK, the CLAP SDK
and CLAP helpers. Their exact commits are listed in `plugins/SOURCES.json`.
These dependencies are downloaded into `.tools` and retain their respective
upstream copyright notices and licenses:

- iPlug2: https://github.com/iPlug2/iPlug2
- VST3 SDK: https://github.com/steinbergmedia/vst3sdk
- CLAP: https://github.com/free-audio/clap
- CLAP helpers: https://github.com/free-audio/clap-helpers

Rust dependencies are listed in `Cargo.lock`; their licenses and source are
provided by their upstream packages. WebAssembly bindings use wasm-bindgen.
Windows WebView integration additionally uses Microsoft's Windows Implementation
Library and WebView2 SDK/runtime under their upstream terms.

The independent verification tools use Unicorn and Capstone; they are not part
of the real-time plug-in runtime.
