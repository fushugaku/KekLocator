use keklocator_engine::{daw::*, Engine};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};}
struct Count;
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = TRACK.try_with(|t| {
            if t.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = TRACK.try_with(|t| {
            if t.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Count = Count;

#[test]
fn disabled_sequencer_preserves_verified_48k_dsp() {
    let mut daw = DawProcessor::new(48000.).unwrap();
    let mut reference = Engine::new().unwrap();
    reference
        .apply_preset(daw.parameters[..20].try_into().unwrap(), 0.)
        .unwrap();
    reference.set_sync(true, 123.).unwrap();
    let mut frame = 0;
    for block in [1, 17, 128, 511].into_iter().cycle().take(100) {
        let mut input: Vec<[f32; 2]> = (0..block)
            .map(|i| {
                let n = (frame + i) as f32;
                [(n * 0.071).sin() * 0.12, (n * 0.113).sin() * 0.09]
            })
            .collect();
        let mut expected = vec![[0.; 2]; block];
        reference.process(&input, &mut expected).unwrap();
        daw.process(&mut input, 123., frame as f64 / 48000. * 123. / 60., true)
            .unwrap();
        assert_eq!(input, expected);
        frame += block;
    }
}
#[test]
fn daw_position_controls_sequence_on_seek_loop_tempo_and_stop() {
    let mut d = DawProcessor::new(48000.).unwrap();
    d.set_parameter(SEQ_ENABLE, 1.).unwrap();
    d.set_parameter(SEQ_RATE, 0.3).unwrap();
    for (beat, bpm, step) in [
        (0., 120., 0),
        (0.25, 120., 1),
        (1.25, 160., 5),
        (4., 90., 0),
        (-0.25, 100., 15),
    ] {
        d.process(&mut [[0.; 2]], bpm, beat, true).unwrap();
        assert_eq!(d.step, step);
        assert_eq!(d.tempo, bpm);
        assert_eq!(d.engine.tempo, bpm as f32);
    }
    d.process(&mut [[0.; 2]; 64], 120., 2., false).unwrap();
    assert_eq!(d.step, -1);
}
fn render_free(chunk: usize) -> Vec<[f32; 2]> {
    let mut d = DawProcessor::new(48000.).unwrap();
    d.set_parameter(SEQ_ENABLE, 1.).unwrap();
    d.set_parameter(SEQ_HOST, 0.).unwrap();
    d.set_parameter(SEQ_RATE, 0.3).unwrap();
    let mut out = vec![[0.; 2]; 96001];
    for block in out.chunks_mut(chunk) {
        d.process(block, 91., -12., false).unwrap();
    }
    out
}
#[test]
fn free_sequence_keeps_time_across_host_buffer_sizes() {
    let expected = render_free(128);
    for chunk in [1, 17, 511] {
        let actual = render_free(chunk);
        let delta = actual
            .iter()
            .flatten()
            .zip(expected.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(delta < 0.00001, "chunk={chunk} delta={delta}");
    }
}
#[test]
fn supported_sample_rates_have_stable_buffers_and_reported_latency() {
    for rate in [
        8000., 22050., 44100., 48000., 88200., 96000., 176400., 192000., 384000.,
    ] {
        let mut d = DawProcessor::new(rate).unwrap();
        d.set_parameter(BYPASS, 1.).unwrap();
        let n = (rate * 0.04) as usize;
        let mut out = vec![[0.; 2]; n];
        out[64] = [1., -0.5];
        let mut offset = 0;
        for block in out.chunks_mut(127) {
            d.process(block, 120., offset as f64 / rate * 2., true)
                .unwrap_or_else(|e| panic!("rate={rate}: {e}"));
            offset += block.len();
        }
        assert!(out.iter().flatten().all(|v| v.is_finite()));
        let (peak, amplitude) = out
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a[0].abs().total_cmp(&b[0].abs()))
            .unwrap();
        assert!(
            (peak as i32 - 64 - d.latency() as i32).abs() <= 2,
            "rate={rate} peak={peak} latency={}",
            d.latency()
        );
        assert!(
            amplitude[0] > 0.1 && amplitude[0] <= 1.01,
            "rate={rate} amplitude={amplitude:?}"
        );
    }
}
#[test]
fn audio_callback_allocates_nothing_with_sequence_and_tempo_changes() {
    for rate in [44100., 48000., 96000., 192000.] {
        let mut d = DawProcessor::new(rate).unwrap();
        d.set_parameter(SEQ_ENABLE, 1.).unwrap();
        d.set_parameter(keklocator_engine::extended::MODE, 1.)
            .unwrap();
        d.set_parameter(keklocator_engine::extended::DUCK_AMOUNT, 0.8)
            .unwrap();
        d.set_parameter(keklocator_engine::extended::MOD_ENABLE, 1.)
            .unwrap();
        d.set_parameter(keklocator_engine::extended::LOCK_MASK, 31. / 31.)
            .unwrap();
        let mut buffer = [[0.; 2]; 257];
        d.process(&mut buffer, 120., 0., true).unwrap();
        ALLOCATIONS.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        let mut ok = true;
        for i in 0..20 {
            if i == 10 {
                ok &= d.program(18).is_ok();
            }
            buffer.fill([0.; 2]);
            ok &= d
                .process(&mut buffer, 120. + i as f64, i as f64 * 0.25, true)
                .is_ok();
        }
        TRACK.with(|t| t.set(false));
        assert!(ok);
        ALLOCATIONS.with(|n| assert_eq!(n.get(), 0, "sample rate {rate}"));
    }
}

#[test]
fn factory_program_change_preserves_clock_settings_and_can_morph() {
    let mut d = DawProcessor::new(48000.).unwrap();
    d.set_parameter(SEQ_ENABLE, 1.).unwrap();
    d.set_parameter(MORPH, 0.1).unwrap();
    assert!(d.program(0).unwrap());
    assert_eq!(d.parameters[SEQ_ENABLE], 1.);
    assert!(d.engine.morph_progress() < 1.);
    assert!(!d.program(127).unwrap());
    let mut audio = vec![[0.; 2]; 48100];
    d.process(&mut audio, 120., 0., true).unwrap();
    assert_eq!(d.engine.morph_progress(), 1.);
    assert_eq!(d.engine.values[19], 0.);
}
