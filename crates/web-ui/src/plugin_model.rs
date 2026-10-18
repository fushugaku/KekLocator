//! Editor state lives off the audio thread. Sound and pattern banks are independent.
use keklocator_engine::{daw, extended as x};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Sound,
    Pattern,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Patch {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub values: Vec<f32>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Library {
    pub patches: Vec<Patch>,
    pub favorites: Vec<String>,
}
impl Library {
    pub fn valid(&self) -> bool {
        self.patches.len() <= 1024
            && self.favorites.len() <= 2048
            && self.patches.iter().all(|p| {
                p.id.len() <= 128
                    && !p.name.trim().is_empty()
                    && p.name.len() <= 240
                    && migrate(&p.values).is_some()
            })
    }
}
pub fn migrate(values: &[f32]) -> Option<[f32; daw::PARAM_COUNT]> {
    if ![64, daw::PARAM_COUNT].contains(&values.len())
        || values
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
    {
        return None;
    }
    let mut p = daw::defaults();
    p[..values.len()].copy_from_slice(values);
    Some(p)
}
pub fn belongs(kind: Kind, id: usize) -> bool {
    match kind {
        Kind::Sound => id < 24 || (daw::MORPH..=x::DUCK_RELEASE).contains(&id),
        Kind::Pattern => (24..daw::MORPH).contains(&id) || id >= x::SWING,
    }
}
pub struct Model {
    pub values: [f32; daw::PARAM_COUNT],
    pub library: Library,
    pub selected: usize,
    pub kind: Kind,
    pub search: String,
    pub favorites_only: bool,
    pub active: String,
    pub bank_open: bool,
    pub a: bool,
    other: [f32; daw::PARAM_COUNT],
    undo: VecDeque<[f32; daw::PARAM_COUNT]>,
    redo: Vec<[f32; daw::PARAM_COUNT]>,
    gesture: Option<[f32; daw::PARAM_COUNT]>,
}
impl Default for Model {
    fn default() -> Self {
        let values = daw::defaults();
        Self {
            values,
            other: values,
            library: Library::default(),
            selected: 0,
            kind: Kind::Sound,
            search: String::new(),
            favorites_only: false,
            active: String::new(),
            bank_open: false,
            a: true,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
        }
    }
}
impl Model {
    pub fn begin(&mut self) {
        if self.gesture.is_none() {
            self.gesture = Some(self.values);
        }
    }
    pub fn finish(&mut self) {
        if let Some(before) = self.gesture.take() {
            if before != self.values {
                if self.undo.len() == 64 {
                    self.undo.pop_front();
                }
                self.undo.push_back(before);
                self.redo.clear();
            }
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) -> bool {
        self.finish();
        if let Some(p) = self.undo.pop_back() {
            self.redo.push(self.values);
            self.values = p;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        self.finish();
        if let Some(p) = self.redo.pop() {
            self.undo.push_back(self.values);
            self.values = p;
            true
        } else {
            false
        }
    }
    pub fn apply(&mut self, values: &[f32], kind: Kind) -> bool {
        let Some(p) = migrate(values) else {
            return false;
        };
        self.begin();
        for (id, v) in p.into_iter().enumerate() {
            if belongs(kind, id) {
                self.values[id] = v;
            }
        }
        self.finish();
        true
    }
    pub fn swap(&mut self) {
        self.finish();
        self.begin();
        for id in 0..daw::PARAM_COUNT {
            if belongs(Kind::Sound, id) {
                std::mem::swap(&mut self.values[id], &mut self.other[id]);
            }
        }
        self.a = !self.a;
        self.finish();
    }
    pub fn copy_ab(&mut self) {
        self.other = self.values;
    }
    pub fn save(&mut self, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() || name.len() > 240 || self.library.patches.len() >= 1024 {
            return None;
        }
        let mut n = 1;
        while self
            .library
            .patches
            .iter()
            .any(|p| p.id == format!("user-{n}"))
        {
            n += 1;
        }
        let id = format!("user-{n}");
        self.library.patches.push(Patch {
            id: id.clone(),
            name: name.into(),
            kind: self.kind,
            values: self.values.to_vec(),
        });
        self.active = id.clone();
        Some(id)
    }
    pub fn favorite(&mut self, id: &str) {
        if let Some(i) = self.library.favorites.iter().position(|v| v == id) {
            self.library.favorites.remove(i);
        } else {
            self.library.favorites.push(id.into());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_projects_default_to_original_and_never_enable_locks() {
        let mut old = vec![0.25; 64];
        old[24] = 1.;
        let p = migrate(&old).unwrap();
        assert_eq!(p[x::MODE], 0.);
        assert_eq!(p[x::MOD_ENABLE], 0.);
        assert_eq!(p[x::PROBABILITY], 1.);
        old[2] = f32::NAN;
        assert!(migrate(&old).is_none());
    }
    #[test]
    fn history_groups_drag_and_ab_keeps_pattern() {
        let mut m = Model::default();
        let before = m.values;
        m.begin();
        for n in 0..100 {
            m.values[3] = n as f32 / 100.;
        }
        m.finish();
        assert!(m.undo());
        assert_eq!(m.values, before);
        assert!(!m.undo());
        assert!(m.redo());
        m.copy_ab();
        m.values[3] = 0.1;
        m.values[31] = 0.75;
        m.swap();
        assert_eq!(m.values[3], 0.99);
        assert_eq!(m.values[31], 0.75);
        m.swap();
        assert_eq!(m.values[3], 0.1);
    }
    #[test]
    fn sound_and_pattern_banks_do_not_overwrite_each_other() {
        let mut m = Model::default();
        let mut patch = m.values;
        patch[3] = 0.01;
        patch[31] = 0.99;
        patch[x::SWING] = 0.4;
        let old = m.values;
        m.apply(&patch, Kind::Sound);
        assert_eq!(m.values[3], 0.01);
        assert_eq!(m.values[31], old[31]);
        m.apply(&patch, Kind::Pattern);
        assert_eq!(m.values[31], 0.99);
        assert_eq!(m.values[x::SWING], 0.4);
        let id = m.save("  My pattern  ").unwrap();
        m.favorite(&id);
        let text = serde_json::to_string(&m.library).unwrap();
        let l: Library = serde_json::from_str(&text).unwrap();
        assert!(l.valid());
        assert_eq!(l.patches[0].name, "My pattern");
        assert_eq!(l.favorites, vec![id]);
    }
}
