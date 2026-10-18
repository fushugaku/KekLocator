mod server;
use keklocator_engine::{parameters::PARAMETERS, presets::PresetBank, Engine};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("serve") {
        "serve"=>{let port=option(&args,"--port").unwrap_or("4173").parse()?;let dir=option(&args,"--dir").unwrap_or("dist");server::serve(dir,port)?;},
        "render"=>render(&args)?,"inspect"|"bench"=>inspect()?,"verify"=>verify()?,
        "bank"=>println!("{}",PresetBank::factory().to_json()),
        _=>return Err("Use: keklocator serve [--port 4173] | render input.wav output.wav [--preset NAME] [--tail 4] | inspect | verify | bank".into()),
    }
    Ok(())
}
fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].as_str())
}
fn preset(engine: &mut Engine, name: &str) -> Result<()> {
    let bank = PresetBank::factory();
    let p = bank
        .presets
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or("Preset not found")?;
    engine.apply_preset(p.values, 0.0)?;
    Ok(())
}
fn inspect() -> Result<()> {
    let started = std::time::Instant::now();
    let mut engine = Engine::new()?;
    println!(
        "DSP initialized in {:?}; {} bytes allocated; {} translated routines",
        started.elapsed(),
        engine.machine.heap_cursor,
        keklocator_engine::FIRMWARE_FUNCTIONS.len()
    );
    preset(&mut engine, "GREAT EYE")?;
    let mut input = vec![[0.0; 2]; 4800];
    input[0] = [0.1, 0.1];
    let mut output = vec![[0.0; 2]; input.len()];
    let started = std::time::Instant::now();
    engine.process(&input, &mut output)?;
    let peak = output
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0.0f32, f32::max);
    println!(
        "100 ms rendered in {:?}; peak={peak:.8}; {} instructions",
        started.elapsed(),
        engine.machine.ticks
    );
    for p in PARAMETERS {
        println!(
            "{:2} {:16} {}",
            p.id,
            p.label,
            p.display(engine.values[p.id])
        );
    }
    Ok(())
}
fn render(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        return Err("Use: render input.wav output.wav [--preset NAME] [--tail seconds]".into());
    }
    let mut input = hound::WavReader::open(&args[1])?;
    let spec = input.spec();
    if spec.sample_rate != 48000 || !(1..=2).contains(&spec.channels) {
        return Err("Native renderer accepts mono/stereo WAV at 48 kHz; the web UI also decodes and resamples other formats".into());
    }
    let source: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => input
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f64.powi(spec.bits_per_sample as i32 - 1);
            input
                .samples::<i32>()
                .map(|v| v.map(|v| (v as f64 / scale) as f32))
                .collect::<std::result::Result<_, _>>()?
        }
    };
    let mut engine = Engine::new()?;
    preset(&mut engine, option(args, "--preset").unwrap_or("GREAT EYE"))?;
    let tail: f32 = option(args, "--tail").unwrap_or("4").parse()?;
    if !tail.is_finite() || !(0.0..=60.0).contains(&tail) {
        return Err("Tail must be between 0 and 60 seconds".into());
    }
    let mut frames: Vec<[f32; 2]> = source
        .chunks_exact(spec.channels as usize)
        .map(|v| [v[0], v.get(1).copied().unwrap_or(v[0])])
        .collect();
    frames.resize(frames.len() + (tail * 48000.0) as usize, [0.0; 2]);
    let mut output = vec![[0.0; 2]; frames.len()];
    let start = std::time::Instant::now();
    engine.process(&frames, &mut output)?;
    let samples: Vec<f32> = output.into_iter().flatten().collect();
    std::fs::write(
        &args[2],
        keklocator_engine::wave::encode_wav(&samples, 2, 48000)?,
    )?;
    println!(
        "Rendered {:.2}s in {:?} -> {}",
        frames.len() as f64 / 48000.0,
        start.elapsed(),
        args[2]
    );
    Ok(())
}
fn verify() -> Result<()> {
    let bank = PresetBank::factory();
    let mut engine = Engine::new()?;
    let mut frames = vec![[0.0; 2]; 2400];
    frames[0] = [0.1, -0.1];
    let mut output = vec![[0.0; 2]; frames.len()];
    for p in &bank.presets {
        engine.apply_preset(p.values, 0.0)?;
        engine.process(&frames, &mut output)?;
        if output.iter().flatten().any(|v| !v.is_finite()) {
            return Err(format!("Non-finite audio in {}", p.name).into());
        }
    }
    for id in 0..20 {
        for value in [0.0, 1.0] {
            engine.set_parameter(id, value)?;
            engine.process(&frames, &mut output)?;
            if output.iter().flatten().any(|v| !v.is_finite()) {
                return Err(format!("Non-finite audio at parameter {id} = {value}").into());
            }
        }
        engine.set_parameter(id, PARAMETERS[id].normalized_default())?;
    }
    engine.set_sync(true, 120.0)?;
    engine.set_sync(false, 120.0)?;
    let encoded = bank.to_elb()?;
    let decoded = PresetBank::from_elb(&encoded)?;
    assert_eq!(decoded.presets.len(), bank.presets.len());
    println!("PASS: {} original presets, all 20 parameter endpoints, sync, ELB roundtrip; finite stereo output",bank.presets.len());
    Ok(())
}
