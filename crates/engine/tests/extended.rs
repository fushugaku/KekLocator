use keklocator_engine::{daw::*, extended as x, Engine};
#[test]
fn original_ignores_all_extensions_even_when_their_controls_are_saved() {
    let mut a = DawProcessor::new(48000.).unwrap();
    let mut b = DawProcessor::new(48000.).unwrap();
    for id in 65..PARAM_COUNT {
        b.set_parameter(id, 0.83).unwrap();
    }
    for n in 0..60 {
        let mut input = [[0.1, -0.08]; 257];
        let mut expected = input;
        a.process(&mut expected, 123., n as f64 * 0.1, true)
            .unwrap();
        b.process(&mut input, 123., n as f64 * 0.1, true).unwrap();
        assert_eq!(input, expected);
    }
}
#[test]
fn swing_probability_and_ratchets_use_absolute_musical_time() {
    let p = x::position(0.25, 3, 16, 1.);
    assert_eq!(p.step, 0);
    assert!((p.phase - 2. / 3.).abs() < 1e-8);
    assert_eq!(x::position(0.375, 3, 16, 1.).step, 1);
    assert_eq!(x::position(0.5, 3, 16, 1.).step, 2);
    assert_eq!(x::position(-0.01, 3, 16, 1.).step, 15);
    let a: Vec<_> = (-64..64).map(|s| x::accepts(1337, s, 0.5)).collect();
    let b: Vec<_> = (-64..64)
        .rev()
        .map(|s| x::accepts(1337, s, 0.5))
        .rev()
        .collect();
    assert_eq!(a, b);
    let count = a.iter().filter(|&&v| v).count();
    assert!((35..95).contains(&count));
    assert!((-20..20).all(|s| !x::accepts(0, s, 0.) && x::accepts(0, s, 1.)));
    let mut seq = sequence(&defaults());
    seq.enabled = true;
    let mut p = x::position(0.08, 3, 16, 0.);
    assert_ne!(x::sample(&seq, p, 120., 1., 1), [0.; 2]);
    assert_eq!(x::sample(&seq, p, 120., 0.1, 1), [0.; 2]);
    p.phase = 0.5;
    assert_eq!(x::sample(&seq, p, 120., 1., 2), [0.; 2]); // exact second trigger, 2ms attack
}
#[test]
fn effect_locks_modulate_external_audio_without_enabling_note_source_and_restore() {
    let mut d = DawProcessor::new(48000.).unwrap();
    d.set_parameter(x::MODE, 1.).unwrap();
    d.set_parameter(x::MOD_ENABLE, 1.).unwrap();
    d.set_parameter(SEQ_RATE, 0.3).unwrap();
    let base = d.parameters;
    d.set_parameter(x::LOCK_MASK, 31. / 31.).unwrap();
    for k in 0..5 {
        d.set_parameter(x::lock_value(0, k), if k == 4 { 1. } else { 0.25 })
            .unwrap();
    }
    d.process(&mut [[0.1, -0.1]; 24], 120., 0., true).unwrap();
    assert_eq!(d.step, 0);
    assert_eq!(d.parameters[SEQ_ENABLE], 0.);
    for (k, id) in x::LOCK_IDS.into_iter().enumerate() {
        assert_eq!(d.engine.values[id], if k == 4 { 1. } else { 0.25 });
        assert_eq!(d.parameters[id], base[id]);
    }
    d.process(&mut [[0.1, -0.1]; 24], 120., 0.25, true).unwrap();
    for id in x::LOCK_IDS {
        assert_eq!(d.engine.values[id], base[id]);
    }
    d.process(&mut [[0.; 2]; 24], 120., 0., true).unwrap();
    d.set_parameter(x::MODE, 0.).unwrap();
    d.process(&mut [[0.; 2]; 24], 120., 0., true).unwrap();
    for id in x::LOCK_IDS {
        assert_eq!(d.engine.values[id], base[id]);
    }
}
#[test]
fn ducking_changes_only_final_wet_mix_and_never_feedback_memory() {
    for mix in [0., 1.] {
        let mut a = Engine::new().unwrap();
        let mut b = Engine::new().unwrap();
        for d in [&mut a, &mut b] {
            d.set_parameter(18, mix).unwrap();
            d.set_parameter(3, 0.).unwrap();
            d.set_parameter(9, 0.7).unwrap();
        }
        for d in [&mut a, &mut b] {
            let mut silence = [[0.; 2]; 24000];
            d.process(&[[0.; 2]; 24000], &mut silence).unwrap();
        }
        b.wet_gain = 0.25;
        let mut wet_energy = 0.;
        for block in 0..1200 {
            let input = std::array::from_fn::<_, 24, _>(|i| {
                let t = (block * 24 + i) as f32;
                [(t * 0.087).sin() * 0.03, (t * 0.049).sin() * 0.02]
            });
            let mut expected = [[0.; 2]; 24];
            let mut actual = expected;
            a.process(&input, &mut expected).unwrap();
            b.process(&input, &mut actual).unwrap();
            for (e, v) in expected.iter().flatten().zip(actual.iter().flatten()) {
                if mix == 0. {
                    assert!((e - v).abs() < 1e-7, "dry endpoint residual: {v} vs {e}");
                } else {
                    wet_energy += e.abs();
                    assert!((v - e * 0.25).abs() < 3e-8, "{v} vs {e}");
                }
            }
        }
        if mix == 1. {
            assert!(wet_energy > 0.01);
        }
        b.wet_gain = 1.;
        // Drain the previous 23 frames; subsequent audio must match bit for bit.
        for block in 0..80 {
            let mut e = [[0.; 2]; 24];
            let mut v = e;
            a.process(&[[0.; 2]; 24], &mut e).unwrap();
            b.process(&[[0.; 2]; 24], &mut v).unwrap();
            if block > 0 {
                assert_eq!(v, e);
            }
        }
    }
}
fn render(chunk: usize) -> Vec<[f32; 2]> {
    let mut d = DawProcessor::new(48000.).unwrap();
    for (id, v) in [
        (x::MODE, 1.),
        (SEQ_ENABLE, 1.),
        (SEQ_HOST, 0.),
        (SEQ_RATE, 0.3),
        (x::SWING, 0.72),
        (x::DUCK_AMOUNT, 0.7),
        (x::MOD_ENABLE, 1.),
    ] {
        d.set_parameter(id, v).unwrap();
    }
    for s in 0..16 {
        d.set_parameter(x::PROBABILITY + s, 0.65).unwrap();
        d.set_parameter(x::RATCHET + s, 3. / 7.).unwrap();
        d.set_parameter(x::GATE + s, 0.6).unwrap();
    }
    let mut audio = vec![[0.; 2]; 70001];
    for part in audio.chunks_mut(chunk) {
        d.process(part, 91., 0., false).unwrap();
    }
    audio
}
#[test]
fn extended_offline_render_does_not_depend_on_host_buffer_size() {
    let a = render(257);
    for chunk in [1, 511] {
        let b = render(chunk);
        let delta = a
            .iter()
            .flatten()
            .zip(b.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(delta < 1e-5, "{chunk}: {delta}");
    }
}
#[test]
fn hosts_display_and_accept_physical_units() {
    let mut p = defaults();
    p[DELAY_SYNC] = 0.;
    for (id, text, want) in [
        (3, "500 ms", 0.7),
        (3, "0.5 s", 0.7),
        (22, "-6 dB", 0.6),
        (23, "-3 dB", 0.75),
        (4, "440 Hz", 59. / 127.),
        (7, "+6 st", 0.75),
        (x::DUCK_ATTACK, "10 ms", 0.5),
        (x::SWING, "25%", 0.5),
        (x::RATCHET, "4", 3. / 7.),
    ] {
        let v = x::parse(id, text, &p).unwrap();
        assert!((v - want).abs() < 1e-5, "{id}: {text} -> {v}");
    }
    p[DELAY_SYNC] = 1.;
    let value = x::parse(3, "1/4", &p).unwrap();
    assert_eq!(x::display(3, value, &p, 120.), "1/4");
    assert_eq!(x::parse(64, "Original", &p), Some(0.));
    assert!(x::parse(22, "nan", &p).is_none());
}
#[test]
fn extended_morph_is_not_interrupted_by_unlocked_controls() {
    let mut a = DawProcessor::new(48000.).unwrap();
    let mut b = DawProcessor::new(48000.).unwrap();
    b.set_parameter(x::MODE, 1.).unwrap();
    let mut p: [f32; 20] = a.parameters[..20].try_into().unwrap();
    p[3] = 0.2;
    p[9] = 0.3;
    p[4] = 0.4;
    p[7] = 0.6;
    p[2] = 0.7;
    a.preset(p, 0.1).unwrap();
    b.preset(p, 0.1).unwrap();
    let mut expected = vec![[0.01, -0.01]; 6000];
    let mut actual = expected.clone();
    a.process(&mut expected, 120., 0., true).unwrap();
    b.process(&mut actual, 120., 0., true).unwrap();
    assert_eq!(actual, expected);
    p[2] = 0.2;
    b.set_parameter(x::MOD_ENABLE, 1.).unwrap();
    b.set_parameter(x::LOCK_MASK, 1. / 31.).unwrap();
    b.set_parameter(x::lock_value(0, 0), 0.8).unwrap();
    b.preset(p, 0.1).unwrap();
    b.process(&mut [[0.; 2]; 24], 120., 0., true).unwrap();
    assert_eq!(b.engine.values[3], 0.8);
    assert!(b.engine.morph_progress() < 1.);
}
