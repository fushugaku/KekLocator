//! Raw Q31 test boundary for comparison with execution of the vendor ARM binary.
use keklocator_engine::{engine::STATE_ADDRESS, Engine};
use serde::Deserialize;
use std::{fs, path::PathBuf};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
struct Plan {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    input: PathBuf,
    output: PathBuf,
    values: [f32; 20],
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default)]
    dump: Option<PathBuf>,
}
#[derive(Deserialize)]
struct Event {
    block: usize,
    #[serde(default)]
    parameter: Option<usize>,
    #[serde(default)]
    value: f32,
    #[serde(default)]
    sync: Option<bool>,
    #[serde(default = "tempo")]
    bpm: f32,
    #[serde(default)]
    preset: Option<[f32; 20]>,
    #[serde(default)]
    seconds: f32,
}
fn tempo() -> f32 {
    120.0
}
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: reference_driver plan.json")?;
    let plan: Plan = serde_json::from_slice(&fs::read(path)?)?;
    for case in plan.cases {
        let mut e = Engine::new()?;
        e.apply_preset(case.values, 0.0)?;
        let input = fs::read(&case.input)?;
        if !input.len().is_multiple_of(192) {
            return Err("Q31 input must contain full 24-frame blocks".into());
        }
        let mut output = Vec::with_capacity(input.len());
        for (block, samples) in input.chunks_exact(192).enumerate() {
            for event in case.events.iter().filter(|v| v.block == block) {
                if let Some(id) = event.parameter {
                    e.set_parameter(id, event.value)?;
                }
                if let Some(enabled) = event.sync {
                    e.set_sync(enabled, event.bpm)?;
                }
                if let Some(values) = event.preset {
                    e.apply_preset(values, event.seconds)?;
                }
            }
            e.advance_controls()?;
            for (i, bytes) in samples.chunks_exact(4).enumerate() {
                e.machine.store(
                    0x20002674 + i as u32 * 4,
                    4,
                    u32::from_le_bytes(bytes.try_into()?) as u64,
                )?;
            }
            e.machine
                .invoke(0x208, &[0x20002674, 0x200027f4, 24], None, 5_000_000)?;
            for i in 0..48 {
                output.extend_from_slice(
                    &(e.machine.load(0x200027f4 + i * 4, 4)? as u32).to_le_bytes(),
                );
            }
        }
        fs::write(case.output, output)?;
        if let Some(prefix) = case.dump {
            fs::write(
                PathBuf::from(format!("{}.dtcm", prefix.display())),
                &e.machine.dtcm,
            )?;
            fs::write(
                PathBuf::from(format!("{}.heap", prefix.display())),
                &e.machine.heap,
            )?;
        }
        eprintln!(
            "{}: {} blocks; {} instructions; state=0x{STATE_ADDRESS:08x}",
            case.name,
            input.len() / 192,
            e.machine.ticks
        );
    }
    Ok(())
}
