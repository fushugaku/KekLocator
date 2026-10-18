pub fn encode_wav(samples: &[f32], channels: u16, sample_rate: u32) -> Result<Vec<u8>, String> {
    if channels == 0 || sample_rate == 0 || !samples.len().is_multiple_of(channels as usize) {
        return Err("Invalid audio layout".into());
    }
    let bytes = samples
        .len()
        .checked_mul(2)
        .filter(|v| *v < u32::MAX as usize - 36)
        .ok_or("WAV exceeds RIFF limit")? as u32;
    let mut out = Vec::with_capacity(bytes as usize + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(bytes + 36).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&bytes.to_le_bytes());
    for &v in samples {
        let value = if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        out.extend_from_slice(&((value * 32767.0).round() as i16).to_le_bytes());
    }
    Ok(out)
}

pub fn demo() -> Vec<f32> {
    let length = 8 * 48000;
    let mut samples = Vec::with_capacity(length * 2);
    let mut noise = 0x13257u32;
    for n in 0..length {
        let t = n as f32 / 48000.0;
        let step = (t / 0.5).floor() as usize;
        let local = t % 0.5;
        let note = [0.0, 7.0, 12.0, 4.0, 0.0, 9.0, 7.0, 12.0][step % 8];
        let f = 110.0 * 2.0f32.powf(note / 12.0);
        let env = (-local * 7.0).exp();
        let pluck = ((t * f * std::f32::consts::TAU).sin()
            + 0.3 * (t * f * 2.0 * std::f32::consts::TAU).sin())
            * env
            * 0.13;
        let kick_time = t % 1.0;
        let kick = ((kick_time * 48.0 + 0.7 * (1.0 - (-kick_time * 20.0).exp()))
            * std::f32::consts::TAU)
            .sin()
            * (-kick_time * 22.0).exp()
            * 0.1;
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        let hat = (noise as i32 as f32 / i32::MAX as f32) * (-local * 140.0).exp() * 0.025;
        samples.push(pluck + kick + hat);
        samples.push(pluck * (0.9 + 0.1 * (t * 0.7).sin()) + kick - hat);
    }
    samples
}
