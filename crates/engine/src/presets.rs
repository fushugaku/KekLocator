use crate::parameters::default_values;
use serde::{Deserialize, Serialize};
pub const CAPACITY: usize = 100;
pub const FACTORY_BACKUP: &[u8] = include_bytes!("../firmware/default-backup.elb");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub values: [f32; 20],
    #[serde(default)]
    pub factory: bool,
}
impl Preset {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.chars().count() > 48 {
            return Err("Preset name must contain 1–48 characters".into());
        }
        if self
            .values
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Preset parameters must be between 0 and 1".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetBank {
    pub format: String,
    pub presets: Vec<Preset>,
}
impl PresetBank {
    pub fn new(presets: Vec<Preset>) -> Result<Self, String> {
        if presets.is_empty() || presets.len() > CAPACITY {
            return Err("A bank must contain 1–100 presets".into());
        }
        for p in &presets {
            p.validate()?;
        }
        Ok(Self {
            format: "keklocator/1".into(),
            presets,
        })
    }
    pub fn factory() -> Self {
        let mut bank =
            Self::from_elb(FACTORY_BACKUP).expect("checked, embedded original factory bank");
        for p in &mut bank.presets {
            p.factory = true;
        }
        bank
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if !matches!(value.format.as_str(), "keklocator/1" | "echolocator-rust/1") {
            return Err("Unknown preset bank format".into());
        }
        Self::new(value.presets)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serializable presets")
    }
    pub fn from_elb(bytes: &[u8]) -> Result<Self, String> {
        let u32_at = |p: usize| -> u32 { u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) };
        if bytes.len() < 16 || u32_at(0) != 0x1a13bfef {
            return Err("This is not an Echolocator ELB backup".into());
        }
        let version = u32_at(4);
        let count = u32_at(8) as usize;
        let length = match version {
            1 => 50,
            2 | 3 => 52,
            _ => return Err("Unsupported ELB version".into()),
        };
        if count == 0 || count > CAPACITY || bytes.len() != 16 + count * length {
            return Err("ELB length or preset count is invalid".into());
        }
        if crc32(&bytes[16..]) != u32_at(12) {
            return Err("ELB checksum does not match".into());
        }
        let mut presets = Vec::with_capacity(count);
        for row in bytes[16..].chunks_exact(length) {
            let end = row[..16].iter().position(|&v| v == 0).unwrap_or(16);
            let name = String::from_utf8_lossy(&row[..end]).trim().to_owned();
            let mut values = default_values();
            for (i, b) in row[16..].chunks_exact(2).enumerate() {
                let value = u16::from_le_bytes([b[0], b[1]]);
                if value > 4095 {
                    return Err(format!("Parameter outside 12-bit range in {name}"));
                }
                values[i] = value as f32 / 4095.0;
            }
            // Same upgrades performed by firmware 0x6002bfc4 for pre-Dub banks.
            if version <= 2 {
                values[15] = 0.0;
                values[16] = 2047.0 / 4095.0;
            }
            if version == 2 {
                values[17] = 0.0;
            }
            values[19] = 0.0;
            presets.push(Preset {
                name,
                values,
                factory: false,
            });
        }
        Self::new(presets)
    }
    pub fn to_elb(&self) -> Result<Vec<u8>, String> {
        Self::new(self.presets.clone())?;
        let mut payload = Vec::with_capacity(self.presets.len() * 52);
        for p in &self.presets {
            let mut name = [0u8; 16];
            let ascii: String = p
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_graphic() || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .take(16)
                .collect();
            name[..ascii.len()].copy_from_slice(ascii.as_bytes());
            payload.extend_from_slice(&name);
            for value in p.values.iter().take(18) {
                payload.extend_from_slice(&((value * 4095.0).round() as u16).to_le_bytes());
            }
        }
        let mut out = Vec::with_capacity(payload.len() + 16);
        for value in [0x1a13bfef, 3, self.presets.len() as u32, crc32(&payload)] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&payload);
        Ok(out)
    }
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

pub fn magic(seed: u64, base: [f32; 20]) -> [f32; 20] {
    let mut rng = seed | 1;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        ((rng >> 40) as u32 as f32) / 16777215.0
    };
    let mut values = base;
    for i in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
        values[i] = next();
    }
    values[9] = 0.3 + values[9] * 0.45;
    values[2] *= 0.8;
    values[8] *= 0.8;
    values[19] = 0.0;
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_bank_checksum_and_roundtrip() {
        let bank = PresetBank::factory();
        assert_eq!(bank.presets.len(), 51);
        assert!(bank.presets.iter().any(|p| p.name == "GREAT EYE"));
        let restored = PresetBank::from_elb(&bank.to_elb().unwrap()).unwrap();
        for (a, b) in bank.presets.iter().zip(restored.presets.iter()) {
            assert_eq!(a.name, b.name);
            for i in 0..18 {
                assert!((a.values[i] - b.values[i]).abs() <= 1.0 / 4095.0);
            }
        }
    }
    #[test]
    fn corruption_is_rejected() {
        let mut bytes = FACTORY_BACKUP.to_vec();
        bytes[100] ^= 1;
        assert!(PresetBank::from_elb(&bytes).is_err());
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;
    #[test]
    fn imports_legacy_bank_and_exports_new_name() {
        let bank = PresetBank::factory();
        let legacy = bank.to_json().replace("keklocator/1", "echolocator-rust/1");
        let migrated = PresetBank::from_json(&legacy).unwrap();
        assert_eq!(migrated.format, "keklocator/1");
        assert_eq!(migrated.presets.len(), 51);
        for (before, after) in bank.presets.iter().zip(&migrated.presets) {
            assert_eq!(before.name, after.name);
            assert_eq!(before.values, after.values);
        }
    }
}
