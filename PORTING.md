# Firmware provenance and translation

KekLocator uses a static Rust translation of **Erica Synths Echolocator 1.05**,
released on September 29, 2026 and retrieved on October 16, 2026. The name
KekLocator refers to this software port, not to a different manufacturer firmware.

Sources: [product](https://www.ericasynths.lv/echolocator-3229/),
[UF2](https://www.ericasynths.lv/service/file/download/product_id/784/file_id/664/),
[manual](https://www.ericasynths.lv/service/file/download/product_id/784/file_id/571/),
[factory bank](https://www.ericasynths.lv/service/file/download/product_id/784/file_id/577/).

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| `echolocator_1.05.uf2` | 499712 | `da9447eb3e4dd978a56b38474ec22fc79bdbc780f5da154f85b19fac04a89304` |
| Extracted `echolocator-1.05.bin` | 249856 | `9cea9f9a998ac7bf05297821290d36ef2843ad72b10f0da583765cc914c58211` |
| `default-backup.elb` | 2668 | `1329d0c4fa7323f2d3ba61a4a0112f0264bc7ff25db071ed9fd741c002534982` |

Hashes were computed locally. `tools/prepare-firmware.py` downloads and verifies
these inputs. Binaries remain outside Git; metadata is in
`crates/engine/firmware/source.json`.

## Memory map

| Region / entry point | Address / size |
| --- | --- |
| Flash payload | `0x60020000`, `0x3d000` bytes |
| Initial DTCM | Flash `0x60057398` → `0x20000000`, `0x1f60` bytes |
| ITCM code | Flash `0x600592f8` → `0x00000000`, `0x3500` bytes |
| Additional DTCM | Flash `0x6005c7f8` → `0x20001f60`, `0x714` bytes |
| DSP initialization | `0x6002a77c` |
| Digital mixer selection | `0x6002aabc`, argument `0` |
| Main audio handler | ITCM `0x00000208`, 24 stereo frames at 48 kHz |
| DSP structure | `0x20000628` |
| Q31 input / output | `0x20002674` / `0x200027f4` |
| Parameter table | `0x20000088`, 20 records of 72 bytes |
| Settings table | `0x20001234`, 36 records of 32 bytes |
| Normalized parameter setter | `0x6002a7e0` |
| Control loop / morphing | ITCM `0x0000322c` |
| Linear parameter conversion, VFMA | `0x6002cbe0` |
| MIDI CC / Note | `0x600286fc` / `0x60033710` |
| MIDI Clock | `0x60033744` |

The generated translation contains **819 recognized functions, 52,034
instructions and 11,957 blocks**. It compiles to native Rust or WebAssembly;
there is no run-time ARM decoder in the audio processor. Addresses and operations
remain traceable to the original code. Overlapping registers use a byte array.
`Machine` bounds ITCM, DTCM, OCRAM, Flash and a 32 MiB heap; DSP buffers use about
20 MiB. Host implementations replace allocation and basic memory routines.

The adapter converts incoming audio into the original 24-frame blocks and adds
23 frames of latency at 48 kHz. Initial silence lets smoothers settle. Selecting
a preset resets Freeze. Unchanged parameter values do not retrigger callbacks;
some callbacks reset delay and modulation state. Physical conversions use
`f32::mul_add` to preserve the original VFMA rounding. Morphs execute the
translated control loop on each 24-frame block, including its floating-point
accumulation and mode-switch timing. Physical ADC polling is disconnected.

Hardware GPIO, DMA, OLED, UART and the bootloader are not executed. The presence
of translated peripheral code is not a claim that the board is emulated.

## Independent reference

`tools/reference_arm.py` runs the unmodified ARM/Thumb bytes in Unicorn 2.1.4,
restoring ITCM and DTCM from Flash. `UC_CPU_ARM_MAX` supports the required
Thumb-2/DSP/FPv5 instructions; Unicorn's Cortex-M7 model does not support all
firmware f64 operations. This is an instruction-level reference, not a board model.

The reference does not call generated Rust or Ghidra P-code. Audio comparisons
replace allocation/free and the unused analog VCA DAC write; the original ARM
`memcpy` and `memset` still execute. Parameter updates use the original setter
and control loop. MIDI Clock tests provide deterministic millisecond time in
place of SysTick.

`tools/verify_original.py --controls --all` compares raw stereo Q31 before f32
or WAV rounding. `tools/verify_control_messages.py` compares normalized MIDI
values and tempo. `tools/audit_coverage.py` checks hashes and translation
inventory. Run all three with `./tools/verify-original.sh`; see [AUDIT.md](AUDIT.md).

## Regeneration

Normal builds use the committed generated Rust and do not need Ghidra.
Regeneration requires Ghidra 12.1.3, Java 21, Python 3, Capstone 5 `cstool`, and
the original analysis project in the sibling directory
`echolocator-1.05-reverse/ghidra`. That analysis project is not included here.
The script opens it read-only:

```sh
ERIKA_GHIDRA_HOME=/path/to/ghidra ./tools/regenerate.sh
```

`SeedCallbacks.java` finds callback tables; Ghidra extends the analysis;
`ExportPcode.java` exports instructions; `lift_rust.py` emits Rust blocks;
`export_parameters.py` extracts parameter bounds and defaults. Outputs go to
`research/`, which is excluded from Git.

Two corrections to decompiler output are essential: 177 FPv5 VSEL instructions
are decoded from original bytes with Capstone and use actual APSR flags; 481
fused multiply-add instructions use Rust `mul_add` with the correct signs and
conditions. The UI and host adapters are separate from generated code.
