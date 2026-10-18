# KekLocator plug-in guide

## Installation and upgrades

Windows x64 provides VST3 and CLAP. Extract the complete package before running
`Install.ps1`. It installs to `%LOCALAPPDATA%\Programs\Common\VST3` and
`%LOCALAPPDATA%\Programs\Common\CLAP`; add these paths to your host's search
paths if needed. The embedded editor uses the Evergreen WebView2 Runtime.

The macOS universal PKG provides VST3, Audio Unit and CLAP for macOS 11 or later,
Apple Silicon and Intel. It installs to the current user's
`~/Library/Audio/Plug-Ins/{VST3,Components,CLAP}`. The plug-ins are signed ad hoc;
the PKG is a local installer without Developer ID signing or notarization.

The effect is named **KekLocator**. Its existing manufacturer identity, VST3 class
IDs, AU codes (`aumf / EcR1 / ErRs`), CLAP ID, and parameter IDs are retained.
Old 64-parameter states migrate to Original mode; 216-parameter states remain
compatible. Remove the previous **Echolocator** bundle of each format before
installing the renamed version: keep only one copy of each identity installed.
Close the DAW first, then rescan its plug-ins after updating.

The editor and DSP work offline. `/plugin.html` is a browser preview of the
performance editor; `/` is the full browser instrument.

## Original and Extended

**Original** is the default, including for older projects. It uses the translated
firmware 1.05 DSP and the previous sequencer source. Swing, probability, gate,
ratchets, locks and ducking have no effect in this mode. Their settings remain
saved when switching modes.

**Extended** enables these additions around the original delay DSP. With Source
and additions disabled, the output agrees with Original, including morphs.

## Sequencer and sync

**Source** enables the internal note source. **Host** follows the DAW's tempo,
transport and musical position. Playback starts with the DAW's Play button;
seeking or looping updates the step immediately. Stop prevents new notes and
restores the base lock values. Existing delay tails continue while the host
continues processing the effect.

**Free** uses its own 20–400 BPM clock. The DAW must keep processing audio while
stopped for free playback and tails. **Delay sync** follows the selected clock;
its musical division is independent of the sequencer **Rate**.

Set 1–16 steps, one of 11 rhythmic divisions, notes, velocities and ±24-semitone
transpose. Zero velocity is a rest. In Extended:

- **Swing** delays odd steps by 0–50% of a division without changing pair length.
- **Probability** decides whether the note and its locks fire.
- **Gate** sets note duration within each step or ratchet.
- **Ratchets** makes 1–8 repeats within a step.
- **Seed** and absolute song position determine probability. Replaying the same
  position or rendering offline repeats the decisions independently of block size.

Click a step number to open its inspector. **Effect locks** applies Time,
Feedback, Filter, Pitch and Freeze values for that step. Unlocked steps restore
the main controls' base values. The controls show base values rather than the
temporary modulation. Locks also work on incoming DAW audio with Source off.
A locked parameter interrupts its own morph; unlocked parameters continue moving.

Host tempo is sampled at block boundaries. The original control/DSP engine
retains its 24-frame quantum. Sample-offset host events are delivered at their
recorded offsets around that original engine.

## Wet ducking

Extended offers Amount, Threshold, Attack (1–100 ms) and Release (20–2000 ms).
The detector listens to incoming audio plus Source after Input gain. It reduces
only the final wet signal, preserving dry audio and feedback-buffer contents.
Set Amount to zero to disable it.

## Sounds, patterns, A/B and history

**Bank → Sounds** contains factory and user sounds; **Patterns** contains user
patterns. Search and ★ filter the list. Sounds save effect and ducking settings;
patterns save notes, velocities, clocks, groove, seed and locks. Loading one type
preserves the other. Enter a name and select **Save sound** or **Save pattern**.

All formats share a user library:

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\KekLocator\library.json` |
| macOS | `~/Library/Application Support/KekLocator/library.json` |
| Linux source build | `$XDG_DATA_HOME/KekLocator/library.json` (default `~/.local/share`) |

An existing Echolocator Rust library is read as a fallback; future saves use the
new path. Writes are atomic and run outside the audio thread. Reopening an editor
loads library changes made in another instance.

**A/B** switches sound states while retaining the pattern. **A → B / B → A**
copies the current sound to the other slot. **Undo/Redo**, ⌘Z/⇧⌘Z or
Ctrl+Z/Ctrl+Shift+Z keeps up to 64 editor actions. A slider gesture counts as one
action. Host automation is not added to editor history. A/B and Undo buffers are
local to an open editor; the project stores current parameters.

**Export** saves `keklocator-plugin/2` JSON with all 216 parameters and the library.
Import also accepts previous 64- and 216-parameter Echolocator files. The browser
instrument's existing local presets are read automatically under the same origin.

**Morph** sets a sound transition up to 10 seconds. MIDI Program Change 1–51
recalls factory sounds. CC 70–82 and Note On use the original firmware mappings;
route MIDI into the effect slot in the DAW. Magic and ELB are available in the
full browser instrument.

## Building and testing

`tools/build-plugins.sh` builds the universal macOS bundles;
`tools/build-pkg.py` creates the PKG. `tools/build-plugins.ps1` builds the Windows
x64 ZIP. Both fetch the pinned SDKs and source data.

`tools/verify-plugins.sh` and `tools/verify-windows.ps1` load the actual native
formats, compare audio against the Rust adapter, and check Original/Extended at
44.1, 48, 96 and 192 kHz. They exercise tempo changes, seeking/looping, stopping,
variable blocks, latency, state, legacy migration, timestamped automation and
physical parameter text. Steinberg's official VST3 validator is included.
macOS additionally exercises AU render, MIDI offsets and scheduled ramps.

Format tests are not a promise of testing every DAW. AAX, Windows ARM and Linux
binary releases are not provided. Digital firmware equivalence does not cover
the analog circuitry; remaining hardware differences are in [AUDIT.md](AUDIT.md).
