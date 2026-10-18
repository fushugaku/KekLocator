# Original-mode verification and remaining differences

The digital DSP was independently compared against **Erica Synths Echolocator
firmware 1.05**. The hardware feature port remains incomplete. This document
records the October 17, 2026 firmware baseline; renamed release binaries must
also pass the current build and format checks.

## Comparison boundary

All 976 UF2 blocks were reconstructed and matched to the 249,856-byte ROM
identified in [PORTING.md](PORTING.md). An independent Unicorn 2.1.4 reference
executes original ARM/Thumb instructions, not generated Rust or Ghidra P-code.
It runs initialization, parameter setters, the control loop, morphing and DSP.

The comparison uses **48 kHz stereo Q31, 24-frame blocks**, before f32 conversion
or PCM16 WAV rounding, and the firmware's digital mixer. Physical ADC polling is
disconnected; controls are normalized to 0–1. Test input is exactly representable
in both Q31 and Web Audio. The CPU model is `UC_CPU_ARM_MAX` because this
Unicorn version's Cortex-M7 model lacks required f64 operations.

## Firmware baseline

**187 scenarios, 23,380,800 stereo frames / 46,761,600 channel samples**, covering
487.1 seconds of audio. The main matrix contains 171 cases; 16 additional cases
exercise long delays and dependent controls.

| Check | Result |
| --- | --- |
| Native Rust versus original ARM Q31 | Zero differing samples; maximum difference 0 |
| WASM AudioEngine versus original Q31 converted to f32 | Zero differing samples |
| Native/WASM PCM16 for five reference sounds | Zero differences |
| Translation inventory | 819 recognized functions, 52,034 instructions; no missing/duplicate entries |
| All 20 parameter callbacks | Present in the translation |
| Entire hardware feature set | Incomplete |

The scenarios cover all 51 factory sounds; all 20 parameters at 0, 0.5 and 1;
32 combinations of Tape/BBD, filter routing, Shimmer, Pitch/Frequency and Dub;
all 17 delay-sync divisions; silence, impulse and near-full-scale input;
Freeze toggles, parameter changes, 20-second tails, 0.1/1/10-second morphs,
retargeting and Freeze changes during morphing. Extended cases include maximum
Time, the slowest divisions, pitch extremes, Dub Mix, feedback and tempo changes.
WASM comparisons compensate the streaming adapter's fixed 23-frame latency.

MIDI testing covered 329 input packets: 78 CC messages, all 128 notes, velocity
zero, Start and 120 clocks. The 13 standard CC values, note mapping and five
quarter-note tempo measurements agreed at the supported normalized boundary.
For notes from 84 upward the original VFMA can produce a slightly negative Time;
the host clamps it to zero. DIN electrical timing, arbitrary CC remaps,
Program Change, OUT/THRU and the full hardware control state machine were not
independently verified by that test.

Early comparison found and corrected callback retriggering on unchanged values,
double-rounded parameter conversion, host-side morph interpolation and different
MIDI note/clock mapping. Native/WASM agreement alone had not detected those
shared adapter bugs; the independent ARM reference was necessary.

## Remaining differences

| Feature | Status |
| --- | --- |
| Magic random values | Host xorshift and bounds replace firmware rand; mode controls 13/14/17 are not randomized |
| Magic names | Not implemented |
| Individual `.elp` preset files | Not implemented; `.elb` banks and JSON are available |
| Hardware-style factory reset | Not implemented; Clear only resets audio buffers |
| Duplicate remapped CC assignments | Host parser uses the first match; firmware applies all matches |
| Tap, pedal and extended MIDI settings | Host implementations; full original state machine not differentially verified |
| Input/output gain | Browser ranges −24…+6 / −12…0 dB differ from physical controls |
| ADC/DAC and analog VCA | Not modeled; tests use the digital mix route |
| OLED, LEDs, brightness, screensaver, pot catch | Replaced by software UI |
| Footswitch, DIN MIDI and MIDI OUT throttling | Adapted; not verified against physical hardware |
| USB mass storage and UF2 bootloader | Not executed |

Static translation completeness does not prove that every instruction is
reachable from the UI or tested. Continuous parameter combinations and arbitrary
inputs have not been exhaustively enumerated. Bit-for-bit agreement with a
physical Echolocator's analog output has not been established.

## Reproduce

```sh
./tools/build-web.sh
./tools/verify.sh
./tools/verify-original.sh
node tools/verify-original-wasm.mjs
node tools/verify-original-wasm.mjs --daw
```

Rust/rustup, Python 3 with venv and Node.js are required. Unicorn/Capstone versions
are pinned in `tools/reference-requirements.txt`. Ghidra is not required to run
the oracle. Raw audio and JSON reports are generated under `research/` and are
excluded from Git. To run the longer cases separately:

```sh
./tools/verify-original.sh --case extended- --output research/original-reference/extended
node tools/verify-original-wasm.mjs research/original-reference/extended/controls --daw
```

Native format tests compare shipped AU/CLAP/VST3 audio to the same Rust adapter
at four host sample rates in Original and Extended. Windows CI runs VST3/CLAP
and Steinberg's validator. These checks establish the tested format behavior;
they are not a claim of a project/bounce test in every DAW.
