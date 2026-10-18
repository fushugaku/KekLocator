# Changes

## 1.2.0

- Rename the software, plug-in bundles, editor and packages to KekLocator.
- Translate the browser instrument, embedded editor, installer and documentation
  to English, including accessibility labels and error messages.
- Add a Windows x64 VST3/CLAP build, portable installer and native format tests.
- Store the Windows user library under APPDATA; migrate by reading previous
  Echolocator libraries and browser storage as a fallback.
- Keep Original mode, DSP, parameter IDs, native identities and legacy states.
- Publish pinned build recipes and GitHub Actions for reproducible Windows builds.

## 1.1.0 — Echolocator Rust

Extended mode introduced swing, probability, gate, ratchets, deterministic seed,
effect locks, wet ducking, sound/pattern libraries, favorites, A/B and Undo/Redo.
Original remained the default. Added timestamped host automation and AU ramps.

## 1.0.0 — Echolocator Rust

Initial native Rust DSP, browser instrument, embedded WebAssembly editor,
VST3/AU/CLAP wrappers, DAW transport sync and internal sequencer.
