use keklocator_engine::{presets::PresetBank, Engine};

fn source(count: usize) -> Vec<[f32; 2]> {
    (0..count)
        .map(|i| {
            let signal = if i < 1200 {
                ((i as f32 * 0.043).sin() * 0.08) * (-(i as f32) / 300.0).exp()
            } else {
                0.0
            };
            [signal, -signal * 0.7]
        })
        .collect()
}
fn patch() -> Engine {
    let mut e = Engine::new().unwrap();
    let bank = PresetBank::factory();
    e.apply_preset(
        bank.presets
            .iter()
            .find(|p| p.name == "GREAT EYE")
            .unwrap()
            .values,
        0.0,
    )
    .unwrap();
    e
}
#[test]
fn arbitrary_host_chunks_preserve_audio_and_echo_tail() {
    let input = source(12000);
    let mut whole = vec![[0.0; 2]; input.len()];
    let mut chunks = whole.clone();
    patch().process(&input, &mut whole).unwrap();
    let mut engine = patch();
    let mut offset = 0;
    let sizes = [1, 7, 24, 127, 128, 1024];
    let mut index = 0;
    while offset < input.len() {
        let end = (offset + sizes[index % sizes.len()]).min(input.len());
        engine
            .process(&input[offset..end], &mut chunks[offset..end])
            .unwrap();
        offset = end;
        index += 1;
    }
    assert_eq!(
        whole, chunks,
        "Audio must be independent of host block boundaries"
    );
    assert!(whole.iter().flatten().all(|s| s.is_finite()));
    assert!(
        whole[4000..].iter().flatten().any(|s| s.abs() > 1e-5),
        "Original delay must produce a wet tail after the source ends"
    );
}
#[test]
fn bypass_has_fixed_bridge_latency_and_morph_reaches_target() {
    let mut e = patch();
    e.settings.bypass = true;
    let input = source(512);
    let mut output = vec![[0.0; 2]; 512];
    e.process(&input, &mut output).unwrap();
    assert!(output[..23].iter().flatten().all(|&v| v == 0.0));
    assert_eq!(&output[23..], &input[..489]);
    let mut target = e.values;
    target[1] = 0.9;
    target[19] = 1.0;
    e.apply_preset(target, 0.1).unwrap();
    e.process(&vec![[0.0; 2]; 2400], &mut vec![[0.0; 2]; 2400])
        .unwrap();
    assert!((e.morph_progress() - 0.5).abs() < 0.01);
    e.process(&vec![[0.0; 2]; 2400], &mut vec![[0.0; 2]; 2400])
        .unwrap();
    assert_eq!(e.morph_progress(), 1.0);
    // Firmware accumulates binary32 steps and can finish on the following tick.
    e.process(&[[0.0; 2]; 24], &mut [[0.0; 2]; 24]).unwrap();
    assert!((e.values[1] - 0.9).abs() < 1e-6);
    assert_eq!(e.values[19], 0.0);
}

#[test]
fn recalling_unchanged_parameters_preserves_the_running_delay() {
    let input = source(18000);
    let mut a = patch();
    let mut b = patch();
    let mut first_a = vec![[0.0; 2]; 6000];
    let mut first_b = first_a.clone();
    a.process(&input[..6000], &mut first_a).unwrap();
    b.process(&input[..6000], &mut first_b).unwrap();
    b.apply_preset(b.values, 0.0).unwrap();
    let mut tail_a = vec![[0.0; 2]; 12000];
    let mut tail_b = tail_a.clone();
    a.process(&input[6000..], &mut tail_a).unwrap();
    b.process(&input[6000..], &mut tail_b).unwrap();
    assert_eq!(
        tail_a, tail_b,
        "Recalling the same values must retain delay buffers and modulation phase"
    );
}
