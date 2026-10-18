//! The same Rust editor runs in the native WebView and browser preview.
use crate::plugin_model::{self, Kind, Library, Model};
use keklocator_engine::{
    daw::{self, *},
    extended as x,
    parameters::PARAMETERS,
    presets::PresetBank,
    sequencer::LABELS,
};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{Element, Event, HtmlInputElement, HtmlSelectElement, KeyboardEvent};
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=keklocatorPlugin,js_name=send)]
    fn native_send(text: &str);
}
type State = Rc<RefCell<Model>>;
fn doc() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn el(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
}
fn send(v: Value) {
    native_send(&v.to_string());
}
fn text(id: &str, value: &str) {
    if let Some(e) = el(id) {
        e.set_text_content(Some(value));
    }
}
fn attr(id: &str, key: &str, value: &str) {
    if let Some(e) = el(id) {
        let _ = e.set_attribute(key, value);
    }
}
fn disabled(id: &str, value: bool) {
    if let Some(e) = el(id) {
        if value {
            let _ = e.set_attribute("disabled", "");
        } else {
            let _ = e.remove_attribute("disabled");
        }
    }
}
fn hidden(id: &str, value: bool) {
    if let Some(e) = el(id) {
        if value {
            let _ = e.set_attribute("hidden", "");
        } else {
            let _ = e.remove_attribute("hidden");
        }
    }
}
fn listen(target: &web_sys::EventTarget, event: &str, f: impl FnMut(Event) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn note(n: u8) -> String {
    format!(
        "{}{}",
        ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][(n % 12) as usize],
        n as i32 / 12 - 1
    )
}
fn range(id: usize, label: &str) -> String {
    format!("<label class='p-range'><span>{label}<output id='ev-{id}'></output></span><input id='ep-{id}' data-param='{id}' type='range' min='0' max='1' step='0.001' aria-label='{label}'></label>")
}
fn toggle(id: usize, label: &str) -> String {
    format!("<button class='p-button' id='ep-{id}' data-toggle='{id}' aria-pressed='false'>{label}</button>")
}
fn input_value(id: usize, v: f32) -> f32 {
    match id {
        TEMPO => 20. + 380. * v,
        TRANSPOSE => (48. * v - 24.).round(),
        LENGTH => (15. * v).round() + 1.,
        NOTES..=46 => (127. * v).round(),
        x::SEED => x::seed(v) as f32,
        _ => v,
    }
}
fn normalized(id: usize, v: f32) -> f32 {
    match id {
        TEMPO => (v - 20.) / 380.,
        TRANSPOSE => (v + 24.) / 48.,
        LENGTH => (v - 1.) / 15.,
        NOTES..=46 => v / 127.,
        x::SEED => v / 65535.,
        _ => v,
    }
}
fn inspector(s: &Model) {
    let step = s.selected;
    if el("step-inspector")
        .and_then(|e| e.get_attribute("data-step"))
        .as_deref()
        == Some(&step.to_string())
    {
        return;
    }
    let controls=(0..5).map(|k|{let id=x::lock_value(step,k);let label=x::LOCK_NAMES[k];format!("<div class='p-lock'><button data-lock='{k}' id='lock-toggle-{k}' class='p-button' aria-pressed='false'>{label}</button>{}</div>",range(id,label))}).collect::<String>();
    let html=format!("<div class='p-inspector-title'><h3>STEP {:02}</h3><span>Effect locks</span></div><div class='p-groove-step'>{}{}{}</div><div class='p-locks'>{controls}</div>",step+1,range(x::PROBABILITY+step,"Probability"),range(x::GATE+step,"Gate"),range(x::RATCHET+step,"Ratchets"));
    if let Some(e) = el("step-inspector") {
        e.set_inner_html(&html);
        let _ = e.set_attribute("data-step", &step.to_string());
    }
}
fn refresh(s: &State) {
    let s = s.borrow();
    let p = &s.values;
    let extended = p[x::MODE] >= 0.5;
    inspector(&s);
    for id in 0..PARAM_COUNT {
        if let Some(e) = el(&format!("ep-{id}")) {
            if let Some(i) = e.dyn_ref::<HtmlInputElement>() {
                i.set_value(&input_value(id, p[id]).to_string());
                if id == TEMPO {
                    i.set_disabled(p[SEQ_HOST] >= 0.5);
                }
            }
            if e.tag_name() == "BUTTON" {
                let _ =
                    e.set_attribute("aria-pressed", if p[id] >= 0.5 { "true" } else { "false" });
            }
        }
        text(&format!("ev-{id}"), &x::display(id, p[id], p, 120.));
        attr(
            &format!("knob-{id}"),
            "style",
            &format!("--value:{};--angle:{}deg", p[id], -140. + 280. * p[id]),
        );
    }
    for id in ["step-inspector", "extended-controls", "duck-panel"] {
        hidden(id, !extended);
    }
    attr(
        "mode-original",
        "aria-pressed",
        if extended { "false" } else { "true" },
    );
    attr(
        "mode-extended",
        "aria-pressed",
        if extended { "true" } else { "false" },
    );
    attr(
        "p-root",
        "data-mode",
        if extended { "extended" } else { "original" },
    );
    text(
        "seq-clock",
        if p[SEQ_HOST] >= 0.5 { "Host" } else { "Free" },
    );
    text(
        "seq-toggle",
        if p[SEQ_ENABLE] >= 0.5 {
            "■ Source"
        } else {
            "▶ Source"
        },
    );
    if let Some(e) = el("seq-division").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()) {
        e.set_value(&((p[SEQ_RATE] * 10.).round() as usize).to_string());
    }
    let bank = PresetBank::factory();
    let found = bank.presets.iter().position(|preset| {
        preset
            .values
            .iter()
            .zip(p[..20].iter())
            .all(|(a, b)| (a - b).abs() < 0.00001)
    });
    if let Some(e) = el("plugin-preset").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()) {
        e.set_value(&found.map_or("-1".into(), |i| i.to_string()));
    }
    let seq = daw::sequence(p);
    for i in 0..16 {
        text(&format!("seq-note-{i}"), &note(seq.notes[i]));
        attr(
            &format!("seq-step-{i}"),
            "data-muted",
            if i >= seq.length || seq.velocities[i] == 0. {
                "true"
            } else {
                "false"
            },
        );
        attr(
            &format!("seq-step-{i}"),
            "data-selected",
            if i == s.selected { "true" } else { "false" },
        );
        text(
            &format!("step-locks-{i}"),
            if extended && x::mask(p[x::LOCK_MASK + i]) != 0 {
                "●"
            } else {
                ""
            },
        );
    }
    for k in 0..5 {
        let active = x::mask(p[x::LOCK_MASK + s.selected]) & (1 << k) != 0;
        attr(
            &format!("lock-toggle-{k}"),
            "aria-pressed",
            if active { "true" } else { "false" },
        );
        disabled(&format!("ep-{}", x::lock_value(s.selected, k)), !active);
    }
    disabled("undo", !s.can_undo());
    disabled("redo", !s.can_redo());
    attr("ab-a", "aria-pressed", if s.a { "true" } else { "false" });
    attr("ab-b", "aria-pressed", if s.a { "false" } else { "true" });
    text("ab-copy", if s.a { "A → B" } else { "B → A" });
    hidden("library", !s.bank_open);
    attr(
        "bank-toggle",
        "aria-expanded",
        if s.bank_open { "true" } else { "false" },
    );
    text(
        "preset-title",
        &if let Some(patch) = s.library.patches.iter().find(|v| v.id == s.active) {
            patch.name.clone()
        } else {
            found.map_or("Custom".into(), |i| bank.presets[i].name.clone())
        },
    );
}
fn render_bank(s: &State) {
    let s = s.borrow();
    let mut rows = String::new();
    let search = s.search.to_lowercase();
    let mut add = |id: &str, name: &str, source: &str| {
        let favorite = s.library.favorites.iter().any(|v| v == id);
        if !name.to_lowercase().contains(&search) || (s.favorites_only && !favorite) {
            return;
        }
        rows.push_str(&format!("<div class='bank-row'><button class='bank-load' data-load='{}'><strong>{}</strong><span>{source}</span></button><button class='bank-favorite' data-favorite='{}' aria-label='Favorite: {}' aria-pressed='{favorite}'>{}</button></div>",escape(id),escape(name),escape(id),escape(name),if favorite{"★"}else{"☆"}));
    };
    if s.kind == Kind::Sound {
        for (i, p) in PresetBank::factory().presets.iter().enumerate() {
            add(&format!("factory-{i}"), &p.name, "Factory");
        }
    }
    for patch in &s.library.patches {
        if patch.kind == s.kind {
            add(&patch.id, &patch.name, "User");
        }
    }
    if rows.is_empty() {
        rows = "<p class='bank-empty'>No presets</p>".into();
    }
    if let Some(e) = el("bank-results") {
        e.set_inner_html(&rows);
    }
    attr(
        "bank-sounds",
        "aria-pressed",
        if s.kind == Kind::Sound {
            "true"
        } else {
            "false"
        },
    );
    attr(
        "bank-patterns",
        "aria-pressed",
        if s.kind == Kind::Pattern {
            "true"
        } else {
            "false"
        },
    );
    attr(
        "bank-favorites",
        "aria-pressed",
        if s.favorites_only { "true" } else { "false" },
    );
    text(
        "bank-save",
        if s.kind == Kind::Sound {
            "Save sound"
        } else {
            "Save pattern"
        },
    );
}
fn changed(before: &[f32; PARAM_COUNT], s: &State, morph: Option<f32>) {
    let s = s.borrow();
    // Pattern/history changes must not restart preset ramps or release Freeze.
    let preset = morph.filter(|_| before[..20] != s.values[..20]);
    if let Some(seconds) = preset {
        let mut values: [f32; 20] = s.values[..20].try_into().unwrap();
        values[19] = 0.; // The original preset loader always releases Freeze.
        send(json!({"type":"preset","values":values,"seconds":seconds}));
    }
    for id in 0..PARAM_COUNT {
        let restore_freeze = preset.is_some() && id == 19 && s.values[19] >= 0.5;
        if restore_freeze || (before[id] != s.values[id] && !(id < 20 && preset.is_some())) {
            send(json!({"type":"parameter","id":id,"value":s.values[id]}));
        }
    }
}

fn set(s: &State, id: usize, value: f32) {
    if id >= PARAM_COUNT || !value.is_finite() {
        return;
    }
    let value = value.clamp(0., 1.);
    {
        let mut s = s.borrow_mut();
        s.begin();
        s.values[id] = value;
        s.active.clear();
    }
    send(json!({"type":"parameter","id":id,"value":value}));
    refresh(s);
}
fn finish(s: &State) {
    s.borrow_mut().finish();
    refresh(s);
}
fn save_library(s: &State) {
    send(json!({"type":"library-save","library":s.borrow().library}));
}
fn load_patch(s: &State, id: &str) {
    let before = s.borrow().values;
    if let Some(index) = id
        .strip_prefix("factory-")
        .and_then(|i| i.parse::<usize>().ok())
    {
        if let Some(p) = PresetBank::factory().presets.get(index) {
            let mut m = s.borrow_mut();
            let mut values = m.values;
            values[..20].copy_from_slice(&p.values);
            m.apply(&values, Kind::Sound);
            m.active = id.into();
        }
    } else {
        let patch = s
            .borrow()
            .library
            .patches
            .iter()
            .find(|p| p.id == id)
            .cloned();
        if let Some(p) = patch {
            let mut m = s.borrow_mut();
            m.apply(&p.values, p.kind);
            m.active = id.into();
        }
    }
    let morph = if s.borrow().kind == Kind::Sound {
        Some(s.borrow().values[MORPH] * 10.)
    } else {
        None
    };
    changed(&before, s, morph);
    refresh(s);
}
fn history(s: &State, redo: bool) {
    let before = s.borrow().values;
    let ok = if redo {
        s.borrow_mut().redo()
    } else {
        s.borrow_mut().undo()
    };
    if ok {
        changed(&before, s, Some(0.));
        s.borrow_mut().active.clear();
        refresh(s);
    }
}
#[wasm_bindgen]
pub fn mount_plugin() {
    let state: State = Rc::new(RefCell::new(Model::default()));
    let presets = PresetBank::factory()
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "<option value='{i}'>{:02} {}</option>",
                i + 1,
                escape(&p.name)
            )
        })
        .collect::<String>();
    let knobs=[3,9,11,18,2,7,8,1,4,10,5,6].iter().map(|&id|format!("<label class='p-knob' id='knob-{id}'><span>{}</span><i class='knob-ring'><i class='knob-pointer'></i></i><input id='ep-{id}' data-param='{id}' aria-label='{}' type='range' min='0' max='1' step='0.001'><output id='ev-{id}'></output></label>",PARAMETERS[id].label,PARAMETERS[id].label)).collect::<String>();
    let toggles = [
        (0, "Tape / BBD"),
        (14, "Filter series"),
        (13, "Shimmer B"),
        (17, "Frequency"),
        (15, "Dub"),
    ]
    .iter()
    .map(|&(id, label)| toggle(id, label))
    .collect::<String>();
    let steps=(0..16).map(|i|format!("<div class='seq-step' id='seq-step-{i}' data-step='{i}'><button class='step-number' data-select='{i}' aria-label='Edit step {}'>{:02}<span id='step-locks-{i}'></span></button><strong id='seq-note-{i}'></strong><input id='ep-{}' data-param='{}' type='number' min='0' max='127' step='1' aria-label='Step {} note'><input id='ep-{}' data-param='{}' type='range' min='0' max='1' step='0.01' aria-label='Step {} velocity'></div>",i+1,i+1,NOTES+i,NOTES+i,i+1,VELOCITIES+i,VELOCITIES+i,i+1)).collect::<String>();
    let divisions = LABELS
        .iter()
        .enumerate()
        .map(|(i, v)| format!("<option value='{i}'>{v}</option>"))
        .collect::<String>();
    let html = format!(
        r#"<div class='p-shell' id='p-root'>
<header class='p-header'><div class='p-brand'><span class='p-mark'>K</span><div><h1>KEKLOCATOR <small>1.2</small></h1><p>STEREO PRECISION DELAY</p></div></div><div class='p-segment' aria-label='Mode'><button id='mode-original' data-mode='0' aria-pressed='true'>Original</button><button id='mode-extended' data-mode='1' aria-pressed='false'>Extended</button></div><div class='p-host'><span id='host-name'>Native DSP</span><strong id='host-tempo'>120.0 BPM</strong><span id='host-state'>STOPPED</span></div></header>
<div class='p-toolbar'><div class='p-preset-tools'><button id='bank-toggle' aria-expanded='false'>Bank</button><strong id='preset-title'>GREAT EYE</strong><select id='plugin-preset' aria-label='Factory preset'><option value='-1'>Custom</option>{presets}</select></div><div class='p-history'><button id='undo' title='Undo · ⌘Z' aria-label='Undo'>↶</button><button id='redo' title='Redo · ⇧⌘Z' aria-label='Redo'>↷</button><div class='p-segment' aria-label='Sound comparison'><button id='ab-a' aria-pressed='true'>A</button><button id='ab-b' aria-pressed='false'>B</button></div><button id='ab-copy' title='Copy current sound to the other slot'>A → B</button></div></div>
<section class='p-library' id='library' hidden><div class='bank-top'><div class='p-segment'><button id='bank-sounds' aria-pressed='true'>Sounds</button><button id='bank-patterns' aria-pressed='false'>Patterns</button></div><input type='search' id='bank-search' placeholder='Search' aria-label='Search presets'><button id='bank-favorites' aria-pressed='false'>★</button></div><div id='bank-results' class='bank-results'></div><form id='bank-form'><input id='bank-name' type='text' maxlength='120' placeholder='Name' aria-label='Preset name' required><button id='bank-save' type='submit'>Save sound</button></form></section>
<section class='p-instrument'><div class='p-preset-row'><label class='p-morph'>Morph<input id='ep-63' data-param='63' type='range' min='0' max='1' step='0.01' aria-label='Morph time'><output id='ev-63'></output></label><div class='p-peaks'><i id='peak-l'></i><i id='peak-r'></i></div></div><div class='p-knobs'>{knobs}</div><div class='p-performance'>{sync}{freeze}{bypass}<details><summary>Settings</summary><div class='p-extra'>{toggles}{compression}{dubmix}{inputgain}{outputgain}</div></details></div><div class='p-duck' id='duck-panel' hidden><span class='p-section-label'>WET DUCKING <output id='duck-meter'>0%</output></span><div class='p-duck-controls'>{duck}{threshold}{attack}{release}</div></div></section>
<section class='p-sequencer'><div class='p-seq-heading'><div><h2>ORBIT SEQUENCE</h2><span id='seq-position'>16 STEPS</span></div><div class='p-seq-controls'><button id='ep-24' class='p-button p-start' data-toggle='24' aria-pressed='false'><span id='seq-toggle'>▶ Source</span></button><button id='ep-25' class='p-button' data-toggle='25' aria-pressed='true'><span id='seq-clock'>Host</span></button><label class='p-field'><span>BPM</span><input id='ep-26' data-param='26' type='number' min='20' max='400' step='0.1' aria-label='Sequence BPM'></label><label class='p-field'><span>Rate</span><select id='seq-division' aria-label='Step division'>{divisions}</select></label><label class='p-field'><span>Steps</span><input id='ep-30' data-param='30' type='number' min='1' max='16' step='1' aria-label='Sequence length'></label><label class='p-field'><span>Shift</span><input id='ep-29' data-param='29' type='number' min='-24' max='24' step='1' aria-label='Sequence transpose'></label></div></div><div id='extended-controls' class='p-groove' hidden>{modulation}{swing}<label class='p-field'><span>Seed</span><input id='ep-70' data-param='70' type='number' min='0' max='65535' step='1' aria-label='Random seed'></label></div><div class='p-steps'>{steps}</div><div id='step-inspector' class='p-inspector' hidden></div><div class='p-seq-footer'>{level}<span id='plugin-status' role='status'>DAW audio input</span></div></section>
<footer class='p-footer'><span>KekLocator · 1.2.0</span><div><button id='plugin-save'>Export</button><button id='plugin-load'>Import</button><input id='plugin-file' type='file' accept='.json' hidden></div></footer></div>"#,
        sync = toggle(DELAY_SYNC, "Delay sync"),
        freeze = toggle(19, "Freeze"),
        bypass = toggle(BYPASS, "Bypass"),
        compression = range(12, "Compressor"),
        dubmix = range(16, "Dub mix"),
        inputgain = range(INPUT_GAIN, "Input gain"),
        outputgain = range(OUTPUT_GAIN, "Output level"),
        duck = range(x::DUCK_AMOUNT, "Amount"),
        threshold = range(x::DUCK_THRESHOLD, "Threshold"),
        attack = range(x::DUCK_ATTACK, "Attack"),
        release = range(x::DUCK_RELEASE, "Release"),
        modulation = toggle(x::MOD_ENABLE, "Effect locks"),
        swing = range(x::SWING, "Swing"),
        level = range(SEQ_LEVEL, "Level")
    );
    el("app").unwrap().set_inner_html(&html);
    refresh(&state);
    render_bank(&state);
    let document: web_sys::EventTarget = doc().unchecked_into();
    let s = state.clone();
    listen(&document, "input", move |e| {
        if let Some(input) = e
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        {
            if input.id() == "bank-search" {
                s.borrow_mut().search = input.value();
                render_bank(&s);
                return;
            }
            if let Some(id) = input
                .get_attribute("data-param")
                .and_then(|v| v.parse::<usize>().ok())
            {
                set(&s, id, normalized(id, input.value_as_number() as f32));
            }
        }
    });
    let gesture = Rc::new(Cell::new(None));
    let g = gesture.clone();
    let s = state.clone();
    listen(&document, "pointerdown", move |e| {
        if let Some(input) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) {
            if let Some(id) = input
                .get_attribute("data-param")
                .and_then(|v| v.parse::<usize>().ok())
            {
                g.set(Some(id));
                s.borrow_mut().begin();
                send(json!({"type":"gesture-start","id":id}));
            }
        }
    });
    let g = gesture.clone();
    let s = state.clone();
    let window: web_sys::EventTarget = web_sys::window().unwrap().unchecked_into();
    listen(&window, "pointerup", move |_| {
        if let Some(id) = g.take() {
            send(json!({"type":"gesture-end","id":id}));
        }
        finish(&s);
    });
    let s = state.clone();
    listen(&document, "click", move |e| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        if let Ok(Some(b)) = t.closest("[data-toggle]") {
            if let Some(id) = b
                .get_attribute("data-toggle")
                .and_then(|v| v.parse::<usize>().ok())
            {
                let v = if s.borrow().values[id] >= 0.5 { 0. } else { 1. };
                set(&s, id, v);
                finish(&s);
            }
            return;
        }
        if let Ok(Some(b)) = t.closest("[data-select]") {
            if let Some(i) = b
                .get_attribute("data-select")
                .and_then(|v| v.parse::<usize>().ok())
            {
                s.borrow_mut().selected = i.min(15);
                refresh(&s);
            }
            return;
        }
        if let Ok(Some(b)) = t.closest("[data-lock]") {
            if let Some(k) = b
                .get_attribute("data-lock")
                .and_then(|v| v.parse::<usize>().ok())
            {
                let step = s.borrow().selected;
                let id = x::LOCK_MASK + step;
                let mask = x::mask(s.borrow().values[id]) ^ (1 << k);
                set(&s, id, mask as f32 / 31.);
                finish(&s);
            }
            return;
        }
        if let Ok(Some(b)) = t.closest("[data-favorite]") {
            if let Some(id) = b.get_attribute("data-favorite") {
                s.borrow_mut().favorite(&id);
                save_library(&s);
                render_bank(&s);
            }
            return;
        }
        if let Ok(Some(b)) = t.closest("[data-load]") {
            if let Some(id) = b.get_attribute("data-load") {
                load_patch(&s, &id);
            }
            return;
        }
        match t
            .closest("button")
            .ok()
            .flatten()
            .map(|b| b.id())
            .as_deref()
            .unwrap_or("")
        {
            "mode-original" => {
                set(&s, x::MODE, 0.);
                finish(&s);
            }
            "mode-extended" => {
                set(&s, x::MODE, 1.);
                finish(&s);
            }
            "undo" => history(&s, false),
            "redo" => history(&s, true),
            "ab-a" | "ab-b" => {
                let want_a = t.id() == "ab-a";
                if s.borrow().a != want_a {
                    let before = s.borrow().values;
                    s.borrow_mut().swap();
                    changed(&before, &s, Some(0.));
                    refresh(&s);
                }
            }
            "ab-copy" => {
                s.borrow_mut().copy_ab();
                text("plugin-status", "Sound copied");
            }
            "bank-toggle" => {
                let open = !s.borrow().bank_open;
                s.borrow_mut().bank_open = open;
                refresh(&s);
                render_bank(&s);
            }
            "bank-sounds" | "bank-patterns" => {
                s.borrow_mut().kind = if t.id() == "bank-sounds" {
                    Kind::Sound
                } else {
                    Kind::Pattern
                };
                render_bank(&s);
            }
            "bank-favorites" => {
                let value = !s.borrow().favorites_only;
                s.borrow_mut().favorites_only = value;
                render_bank(&s);
            }
            "plugin-save" => {
                send(
                    json!({"type":"save","values":s.borrow().values.as_slice(),"library":s.borrow().library}),
                );
            }
            "plugin-load" => {
                if let Some(i) =
                    el("plugin-file").and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                {
                    i.click();
                }
            }
            _ => {}
        }
    });
    let s = state.clone();
    listen(&document, "submit", move |e| {
        if e.target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .is_some_and(|t| t.id() == "bank-form")
        {
            e.prevent_default();
            let name = el("bank-name")
                .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                .map(|i| i.value())
                .unwrap_or_default();
            if s.borrow_mut().save(&name).is_some() {
                save_library(&s);
                refresh(&s);
                render_bank(&s);
            }
        }
    });
    let s = state.clone();
    listen(&document, "change", move |e| {
        if let Some(select) = e
            .target()
            .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok())
        {
            if select.id() == "seq-division" {
                if let Ok(i) = select.value().parse::<usize>() {
                    set(&s, SEQ_RATE, i as f32 / 10.);
                }
            } else if select.id() == "plugin-preset" {
                s.borrow_mut().kind = Kind::Sound;
                load_patch(&s, &format!("factory-{}", select.value()));
            }
            finish(&s);
        } else if let Some(input) = e
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        {
            if input.id() == "plugin-file" {
                if let Some(file) = input.files().and_then(|f| f.get(0)) {
                    let s = s.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if file.size() > 2. * 1024. * 1024. {
                            text("plugin-status", "File is too large");
                            return;
                        }
                        let result = wasm_bindgen_futures::JsFuture::from(file.text()).await;
                        let parsed = result
                            .ok()
                            .and_then(|v| v.as_string())
                            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
                        if let Some(v) = parsed {
                            if let Some(values) = v["parameters"]
                                .as_array()
                                .and_then(|a| {
                                    a.iter()
                                        .map(|n| n.as_f64().map(|n| n as f32))
                                        .collect::<Option<Vec<_>>>()
                                })
                                .and_then(|a| plugin_model::migrate(&a))
                            {
                                let before = s.borrow().values;
                                {
                                    let mut m = s.borrow_mut();
                                    m.begin();
                                    m.values = values;
                                    m.finish();
                                }
                                changed(&before, &s, Some(0.));
                                if let Ok(library) =
                                    serde_json::from_value::<Library>(v["library"].clone())
                                {
                                    if library.valid() {
                                        s.borrow_mut().library = library;
                                        save_library(&s);
                                    }
                                }
                                refresh(&s);
                                render_bank(&s);
                                return;
                            }
                        }
                        text("plugin-status", "Invalid state file");
                    });
                }
            } else {
                finish(&s);
            }
        }
    });
    let s = state.clone();
    listen(&document, "keydown", move |e| {
        if let Some(k) = e.dyn_ref::<KeyboardEvent>() {
            if k.meta_key() || k.ctrl_key() {
                let editing = k
                    .target()
                    .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                    .is_some_and(|i| ["text", "search"].contains(&i.type_().as_str()));
                if !editing
                    && (k.key().eq_ignore_ascii_case("z") || k.key().eq_ignore_ascii_case("y"))
                {
                    e.prevent_default();
                    history(&s, k.shift_key() || k.key().eq_ignore_ascii_case("y"));
                }
            }
        }
    });
    let s = state.clone();
    listen(&window, "keklocator-plugin", move |e| {
        let Some(t) = e
            .dyn_ref::<web_sys::CustomEvent>()
            .and_then(|e| e.detail().as_string())
        else {
            return;
        };
        let Ok(v) = serde_json::from_str::<Value>(&t) else {
            return;
        };
        match v["type"].as_str().unwrap_or("") {
            "parameters" => {
                if let Some(a) = v["values"].as_array() {
                    let mut m = s.borrow_mut();
                    for (id, n) in a.iter().enumerate().take(PARAM_COUNT) {
                        if let Some(n) = n.as_f64().filter(|n| n.is_finite()) {
                            m.values[id] = (n as f32).clamp(0., 1.);
                        }
                    }
                    drop(m);
                    refresh(&s);
                }
            }
            "parameter" => {
                if let (Some(id), Some(value)) = (v["id"].as_u64(), v["value"].as_f64()) {
                    if (id as usize) < PARAM_COUNT && value.is_finite() {
                        s.borrow_mut().values[id as usize] = (value as f32).clamp(0., 1.);
                        refresh(&s);
                    }
                }
            }
            "library" => {
                if let Ok(library) = serde_json::from_value::<Library>(v["library"].clone()) {
                    if library.valid() {
                        s.borrow_mut().library = library;
                        render_bank(&s);
                    }
                }
            }
            "transport" => {
                let tempo = v["tempo"].as_f64().unwrap_or(120.);
                text("host-tempo", &format!("{tempo:.1} BPM"));
                let playing = v["playing"].as_bool().unwrap_or(false);
                text("host-state", if playing { "PLAYING" } else { "STOPPED" });
                if s.borrow().values[SEQ_HOST] >= 0.5 {
                    if let Some(i) = el("ep-26").and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                    {
                        i.set_value(&format!("{tempo:.1}"));
                    }
                }
                let step = v["step"].as_i64().unwrap_or(-1);
                text(
                    "seq-position",
                    if step >= 0 { "RUNNING" } else { "16 STEPS" },
                );
                for i in 0..16 {
                    attr(
                        &format!("seq-step-{i}"),
                        "data-active",
                        if step == i { "true" } else { "false" },
                    );
                }
                for (ch, id) in [("left", "peak-l"), ("right", "peak-r")] {
                    let n = v[ch].as_f64().unwrap_or(0.).clamp(0., 1.);
                    attr(id, "style", &format!("--peak:{n}"));
                }
                text(
                    "duck-meter",
                    &format!("{:.0}%", v["duck"].as_f64().unwrap_or(0.) * 100.),
                );
                if v["error"].as_i64().is_some_and(|e| e != 0) {
                    text("plugin-status", "Audio engine error");
                } else if (s.borrow().values[SEQ_ENABLE] >= 0.5
                    || (s.borrow().values[x::MODE] >= 0.5
                        && s.borrow().values[x::MOD_ENABLE] >= 0.5))
                    && s.borrow().values[SEQ_HOST] >= 0.5
                    && !playing
                {
                    text("plugin-status", "Waiting for DAW playback");
                } else if step >= 0 {
                    text(
                        "plugin-status",
                        &format!(
                            "Step {} / {}",
                            step + 1,
                            daw::sequence(&s.borrow().values).length
                        ),
                    );
                } else {
                    text("plugin-status", "DAW audio input");
                }
            }
            "status" => {
                if let Some(t) = v["message"].as_str() {
                    text("plugin-status", t);
                }
                if let Some(t) = v["host"].as_str() {
                    text("host-name", t);
                }
            }
            _ => {}
        }
    });
    send(json!({"type":"ready","values":state.borrow().values.as_slice()}));
}
