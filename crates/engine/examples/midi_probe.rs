use keklocator_engine::midi::{note_time, MidiAction, MidiState};
use serde::Deserialize;
use serde_json::json;
#[derive(Deserialize)]
struct Packet {
    data: Vec<u8>,
    time: f64,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("midi_probe packets.json")?;
    let packets: Vec<Packet> = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut midi = MidiState::default();
    let map = std::array::from_fn(|i| 70 + i as u8);
    let mut results = Vec::new();
    for p in packets {
        let result = match midi.message(&p.data, p.time, 1, &map) {
            Some(MidiAction::Tempo(v)) => json!({"tempo":v}),
            Some(MidiAction::Parameter(id, value)) => json!({"parameter":id,"value":value}),
            Some(MidiAction::Note(note)) => json!({"parameter":3,"value":note_time(note)}),
            _ => serde_json::Value::Null,
        };
        results.push(result);
    }
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
