use keklocator_engine::controls::{Pedal, PedalEvent};
use keklocator_engine::{
    midi::{MidiAction, MidiState, TapTempo, CC_ORDER, SYNC_LABELS},
    parameters::{HostSettings, PARAMETERS},
    presets::{magic, Preset, PresetBank},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{Document, Element, Event, HtmlInputElement, KeyboardEvent};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=keklocatorAudio,js_name=command)]
    fn audio_command(text: &str);
    #[wasm_bindgen(js_namespace=keklocatorAudio,js_name=loadFile)]
    fn audio_file(file: &web_sys::File) -> js_sys::Promise;
    #[wasm_bindgen(js_namespace=keklocatorAudio,js_name=loadDemo)]
    fn audio_demo(samples: &js_sys::Float32Array, rate: u32) -> js_sys::Promise;
    #[wasm_bindgen(js_namespace=keklocatorAudio,js_name=download)]
    fn download(bytes: &js_sys::Uint8Array, name: &str, mime: &str);
    #[wasm_bindgen(js_namespace=keklocatorAudio,js_name=midiSupported)]
    fn midi_supported() -> bool;
}
const STORAGE: &str = "keklocator:v1";
const KNOBS: [usize; 12] = [3, 9, 11, 18, 2, 7, 8, 1, 4, 10, 5, 6];
type Shared = Rc<RefCell<State>>;
#[derive(Serialize, Deserialize)]
struct Saved {
    bank: PresetBank,
    active: usize,
    values: [f32; 20],
    settings: HostSettings,
    sync: u8,
    bpm: f32,
}
struct State {
    bank: PresetBank,
    active: usize,
    values: [f32; 20],
    settings: HostSettings,
    sync: u8,
    bpm: f32,
    search: String,
    dirty: bool,
    advanced: bool,
    playing: bool,
    recording: bool,
    loaded: bool,
    live: bool,
    busy: bool,
    status: String,
    error: bool,
    source: String,
    duration: f64,
    progress: f32,
    midi: MidiState,
    tap: TapTempo,
    wave: VecDeque<[f32; 2]>,
    midi_count: usize,
    pedal: Pedal,
    pedal_restore: Option<(String, bool)>,
}
impl State {
    fn new() -> Self {
        let bank = PresetBank::factory();
        let active = bank
            .presets
            .iter()
            .position(|p| p.name == "GREAT EYE")
            .unwrap_or(0);
        let values = bank.presets[active].values;
        let mut s = Self {
            bank,
            active,
            values,
            settings: HostSettings::default(),
            sync: 0,
            bpm: 120.0,
            search: String::new(),
            dirty: false,
            advanced: false,
            playing: false,
            recording: false,
            loaded: false,
            live: false,
            busy: false,
            status: "Ready".into(),
            error: false,
            source: "No audio".into(),
            duration: 0.0,
            progress: 1.0,
            midi: MidiState::default(),
            tap: TapTempo::default(),
            wave: VecDeque::new(),
            midi_count: 0,
            pedal: Pedal::default(),
            pedal_restore: None,
        };
        if let Ok(Some(storage)) = window().local_storage() {
            if let Ok(text) = storage
                .get_item(STORAGE)
                .ok()
                .flatten()
                .or_else(|| storage.get_item("echolocator-rust:v1").ok().flatten())
                .ok_or(())
            {
                if let Ok(saved) = serde_json::from_str::<Saved>(&text) {
                    if PresetBank::new(saved.bank.presets.clone()).is_ok()
                        && saved.active < saved.bank.presets.len()
                        && saved
                            .values
                            .iter()
                            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    {
                        s.bank = saved.bank;
                        s.settings = saved.settings;
                        if s.settings.load_last {
                            s.active = saved.active;
                            s.values = saved.values;
                            s.sync = saved.sync.min(2);
                            s.bpm = saved.bpm.clamp(20.0, 400.0);
                        } else {
                            s.active = 0;
                            s.values = s.bank.presets[0].values;
                        }
                        s.values[19] = 0.0;
                        s.settings.bypass = false;
                    }
                }
            }
        }
        s
    }
    fn save(&self) {
        let saved = Saved {
            bank: self.bank.clone(),
            active: self.active,
            values: self.values,
            settings: self.settings.clone(),
            sync: self.sync,
            bpm: self.bpm,
        };
        if let (Ok(text), Ok(Some(storage))) =
            (serde_json::to_string(&saved), window().local_storage())
        {
            let _ = storage.set_item(STORAGE, &text);
        }
    }
    fn name(&self) -> &str {
        &self.bank.presets[self.active].name
    }
    fn value_text(&self, id: usize) -> String {
        if id == 3 && self.sync > 0 {
            SYNC_LABELS[((self.values[3] * 16.99) as usize).min(16)].into()
        } else if id == 7 && self.values[17] >= 0.5 {
            format!("{:+.1}", PARAMETERS[7].physical(self.values[7]))
        } else {
            PARAMETERS[id].display(self.values[id])
        }
    }
    fn send_all(&self, seconds: f32) {
        send(json!({"type":"preset","values":self.values,"seconds":seconds}));
        self.send_host();
        self.send_sync();
    }
    fn send_host(&self) {
        send(
            json!({"type":"host","input_db":self.settings.input_db,"output_db":self.settings.output_db,"bypass":self.settings.bypass}),
        );
    }
    fn send_sync(&self) {
        send(json!({"type":"sync","enabled":self.sync>0,"bpm":self.bpm}));
    }
    fn set_parameter(&mut self, id: usize, value: f32, midi_out: bool) {
        let value = if PARAMETERS[id].toggle {
            if value >= 0.5 {
                1.0
            } else {
                0.0
            }
        } else {
            value.clamp(0.0, 1.0)
        };
        self.values[id] = value;
        self.dirty = true;
        send(json!({"type":"parameter","id":id,"value":value}));
        if midi_out && self.settings.midi_output {
            if let Some(cc_index) = CC_ORDER.iter().position(|&p| p == id) {
                send(
                    json!({"type":"midi_out","data":[0xb0+(self.settings.midi_out_channel.clamp(1,16)-1),self.settings.midi_cc[cc_index],(value*127.0).round() as u8]}),
                );
            }
        }
    }
    fn select(&mut self, index: usize) {
        if index >= self.bank.presets.len() {
            return;
        }
        self.active = index;
        self.values = self.bank.presets[index].values;
        self.values[19] = 0.0;
        self.dirty = false;
        self.send_all(self.settings.morph_seconds);
        self.status = format!("{}", self.name());
        self.error = false;
        self.save();
    }
    fn tap(&mut self) {
        if let Some(bpm) = self.tap.tap(now()) {
            self.bpm = bpm;
            self.sync = 1;
            self.values[3] = 8.5 / 16.99;
            self.send_sync();
            send(json!({"type":"parameter","id":3,"value":self.values[3]}));
            self.dirty = true;
        }
        self.error = false;
        self.status = "Tap tempo".into();
        self.save();
    }
}
fn window() -> web_sys::Window {
    web_sys::window().expect("browser window")
}
fn doc() -> Document {
    window().document().expect("document")
}
fn now() -> f64 {
    window().performance().map(|v| v.now()).unwrap_or(0.0)
}
fn send(v: Value) {
    audio_command(&v.to_string());
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn el(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
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
fn pressed(id: &str, value: bool) {
    attr(id, "aria-pressed", if value { "true" } else { "false" });
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
fn icon(name: &str) -> String {
    let path:String=match name {
        "play"=>"<path d='m8 5 11 7-11 7Z'/>".into(),"pause"=>"<path d='M8 5v14M16 5v14'/>".into(),
        "folder"=>"<path d='M3 8V5h7l2 3h9v12H3Z'/><path d='M3 11h18'/>".into(),
        "download"=>"<path d='M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5'/>".into(),
        "mic"=>"<rect x='9' y='3' width='6' height='12' rx='3'/><path d='M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8'/>".into(),
        "shuffle"=>"<path d='M3 6h4l10 12h4M3 18h4L17 6h4m-4-4 4 4-4 4m0 4 4 4-4 4'/>".into(),
        "settings"=>"<path d='M4 6h16M4 12h16M4 18h16M8 3v6m8 0v6m-5 0v6'/>".into(),
        "prev"=>"<path d='m14 6-6 6 6 6'/>".into(),"next"=>"<path d='m10 6 6 6-6 6'/>".into(),
        "save"=>"<path d='M4 3h13l4 4v14H4Z'/><path d='M8 3v6h9V3M8 21v-8h9v8'/>".into(),
        _=>"<circle cx='12' cy='12' r='7'/>".into(),
    };
    format!("<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>{path}</svg>")
}
fn knob(s: &State, id: usize) -> String {
    let label = match id {
        4 => "FILTER CUTOFF",
        10 => "FILTER Q",
        18 => "DRY / WET",
        7 => "PITCH SHIFT",
        _ => PARAMETERS[id].label,
    };
    let value = s.values[id];
    let angle = -135.0 + 270.0 * value;
    format!(
        r#"<div class="knob" id="knob-{id}"><label for="parameter-{id}">{label}</label><div class="knob-hit"><svg viewBox="0 0 88 88" aria-hidden="true"><circle class="knob-arc" cx="44" cy="44" r="36" transform="rotate(135 44 44)" stroke-dasharray="169.65 226.2"/><circle id="arc-{id}" class="knob-progress" cx="44" cy="44" r="36" transform="rotate(135 44 44)" stroke-dasharray="{arc} 226.2"/><circle class="knob-body" cx="44" cy="44" r="27"/><circle class="knob-inner" cx="44" cy="44" r="22"/><line id="pointer-{id}" class="knob-pointer" x1="44" y1="20" x2="44" y2="31" transform="rotate({angle} 44 44)"/></svg><input id="parameter-{id}" data-param="{id}" type="range" min="0" max="1" step="0.001" value="{value}" aria-label="{label}" aria-valuetext="{value_text}" title="{label}"/></div><output id="value-{id}" for="parameter-{id}">{value_text}</output></div>"#,
        arc = 169.65 * value,
        value_text = escape(&s.value_text(id))
    )
}
fn small_range(s: &State, id: usize, label: &str) -> String {
    format!("<label class='small-control'><span>{label}<output id='value-{id}'>{}</output></span><input id='parameter-{id}' data-param='{id}' type='range' min='0' max='1' step='0.001' value='{}' aria-label='{label}'></label>",escape(&s.value_text(id)),s.values[id])
}
fn toggle(s: &State, id: usize, label: &str) -> String {
    format!("<div class='setting-row'><span>{label}</span><button id='toggle-{id}' class='switch-value' data-action='toggle' data-id='{id}' aria-pressed='{}'>{}</button></div>",s.values[id]>=0.5,escape(&s.value_text(id)))
}
fn pedal_settings(s: &State) -> String {
    let fields=[("single","Single press",&s.settings.footswitch_single),("double","Double press",&s.settings.footswitch_double),("hold","Hold 2 s",&s.settings.footswitch_hold)].into_iter().map(|(id,label,value)|{
        let options=[("off","Off"),("bypass","Bypass"),("freeze","Freeze"),("next","Next preset"),("previous","Previous preset"),("random","Random preset"),("magic","Magic"),("tap","Tap tempo")].into_iter().filter(|(v,_)|id!="hold"||matches!(*v,"off"|"bypass"|"freeze")).map(|(v,text)|format!("<option value='{v}' {}>{text}</option>",if v==value{"selected"}else{""})).collect::<String>();
        format!("<label>{label}<select data-pedal='{id}' aria-label='Pedal: {label}'>{options}</select></label>")
    }).collect::<String>();
    format!("<div class='pedal-settings'><div><h3>Pedal</h3><button id='pedal-button' class='button' title='Pedal · P'>Pedal · P</button></div>{fields}</div>")
}
fn render(s: &State) -> String {
    let knobs = KNOBS.into_iter().map(|id| knob(s, id)).collect::<String>();
    let toggles = [
        (0, "Delay"),
        (14, "Filter routing"),
        (13, "Shimmer blend"),
        (17, "Shifter"),
        (15, "Dub mode"),
    ]
    .into_iter()
    .map(|(id, label)| toggle(s, id, label))
    .collect::<String>();
    let morph = [0.0, 0.1, 0.2, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0]
        .into_iter()
        .map(|v| {
            format!(
                "<button data-action='morph' data-value='{v}' aria-pressed='{}'>{v}</button>",
                (s.settings.morph_seconds - v).abs() < 0.001
            )
        })
        .collect::<String>();
    let cc=CC_ORDER.iter().enumerate().map(|(index,&id)|format!("<label class='cc-field'><span>{}</span><input data-cc='{index}' type='number' min='0' max='127' value='{}' aria-label='{} MIDI CC'></label>",PARAMETERS[id].label,s.settings.midi_cc[index],PARAMETERS[id].label)).collect::<String>();
    format!(
        r#"
<div class="shell">
 <header class="app-header"><div class="brand"><span class="brand-symbol">K</span><div><h1>KEKLOCATOR<span>1.2</span></h1><p>STEREO PRECISION DELAY</p></div></div><div class="header-right"><span class="connection"><i id="audio-led"></i><span id="engine-label">48 kHz · stereo</span></span><button class="icon-button" data-action="advanced" id="settings-button" aria-expanded="false" aria-controls="advanced-panel" title="Settings">{settings_icon}</button></div></header>
 <div class="workspace">
  <aside class="preset-rack"><div class="rack-heading"><h2>Presets</h2><span id="preset-count"></span></div><input class="search" id="preset-search" type="search" placeholder="Search" aria-label="Search presets"><div class="preset-list" id="preset-list" role="listbox" aria-label="Presets"></div><div class="rack-actions"><button class="button" data-action="save">{save_icon}Save</button><button class="button" data-action="magic" title="Magic · M">{shuffle_icon}Magic</button></div><div class="bank-tools"><button data-action="import-bank">Import</button><button data-action="export-json">JSON ↓</button><button data-action="export-elb">ELB ↓</button></div><div class="preset-order"><button data-action="move-up" title="Move up" aria-label="Move preset up">↑</button><button data-action="move-down" title="Move down" aria-label="Move preset down">↓</button><button data-action="delete" class="delete-preset">Delete</button></div></aside>
  <section class="instrument" aria-label="KekLocator">
   <div class="display"><div class="oled"><div class="oled-top"><span id="sound-mode">TAPE</span><span id="sync-mode-label">FREE</span></div><div class="patch-line"><span id="patch-number">01</span><h2 id="patch-name"></h2><span id="dirty-indicator" aria-label="Unsaved changes">•</span></div><div class="time-line"><strong id="time-display"></strong><span id="tempo-display">120.0 BPM</span></div><div class="morph-track"><i id="morph-progress"></i></div></div><div class="signal-view"><div class="scope-label"><span>SIGNAL</span><span>L / R</span></div><svg class="scope" viewBox="0 0 360 72" preserveAspectRatio="none" aria-label="Stereo signal"><path class="scope-grid" d="M0 18h360M0 54h360M90 0v72M180 0v72M270 0v72"/><path id="scope-l" d="M0 18H360"/><path id="scope-r" d="M0 54H360"/></svg><svg class="echo-map" viewBox="0 0 360 28" aria-label="Tap positions"><path d="M8 14H352"/><circle id="tap-0" cx="40" cy="14" r="3"/><circle id="tap-1" cx="70" cy="14" r="3"/><circle id="tap-2" cx="100" cy="14" r="3"/><circle id="tap-3" cx="130" cy="14" r="3"/><circle id="tap-4" cx="190" cy="14" r="3"/><circle id="tap-5" cx="220" cy="14" r="3"/><circle id="tap-6" cx="250" cy="14" r="3"/><circle id="tap-7" cx="280" cy="14" r="3"/></svg><div class="signal-meters"><span>L</span><div><i id="meter-l"></i></div><span>R</span><div><i id="meter-r"></i></div></div></div></div>
   <div class="knob-grid">{knobs}</div>
   <div class="performance-row"><div class="patch-nav"><button class="icon-button" data-action="previous" title="Previous preset" aria-label="Previous preset">{prev_icon}</button><button class="icon-button" data-action="next" title="Next preset" aria-label="Next preset">{next_icon}</button></div><div class="sync-buttons"><button id="sync-free" data-action="sync" data-value="0" aria-pressed="true">Free</button><button id="sync-tap" data-action="sync" data-value="1" aria-pressed="false">Tap</button><button id="sync-midi" data-action="sync" data-value="2" aria-pressed="false">MIDI</button></div><button class="performance-button" id="tap-button" data-action="tap" title="Tap tempo · T"><i></i>Tap tempo</button><button class="performance-button" id="freeze-button" data-action="freeze" aria-pressed="false" title="Freeze · F"><i></i>Freeze</button><button class="bypass" id="bypass-button" data-action="bypass" aria-pressed="false" title="Bypass · B">Bypass</button></div>
  </section>
 </div>
 <section class="advanced" id="advanced-panel" hidden><div class="advanced-heading"><h2>Effect and controls</h2><button class="icon-button" data-action="advanced" aria-label="Close settings">×</button></div><div class="advanced-grid"><div class="routing-settings">{toggles}{compression}{dub_mix}</div><div class="global-settings"><label class="small-control"><span>Input gain<output id="input-db-value">0 dB</output></span><input id="input-db" data-host="input_db" type="range" min="-24" max="6" step="1" value="0" aria-label="Input gain"></label><label class="small-control"><span>Output level<output id="output-db-value">0 dB</output></span><input id="output-db" data-host="output_db" type="range" min="-12" max="0" step="1" value="0" aria-label="Output level"></label><div class="morph-setting"><span>Morph, s</span><div class="segments morph-options">{morph}</div></div><div class="setting-row"><span>Load last sound on startup</span><button class="switch-value" id="load-last" data-action="load-last" aria-pressed="true">On</button></div><div class="setting-row"><span>Effect buffer</span><button class="button compact" data-action="clear">Clear</button></div><div class="setting-row"><span>WAV tail</span><div class="segments" id="tail-options"><button data-action="tail" data-value="0">0 s</button><button data-action="tail" data-value="4" aria-pressed="true">4 s</button><button data-action="tail" data-value="8">8 s</button><button data-action="tail" data-value="16">16 s</button></div></div></div><div class="midi-settings"><div class="midi-heading"><h3>MIDI</h3><button class="button compact" id="midi-button" data-action="midi">Connect</button></div><p class="midi-status" id="midi-status">{midi_status}</p><div class="midi-channels"><label>IN channel<input id="midi-in-channel" data-setting="midi_channel" type="number" min="0" max="16" value="1" aria-label="MIDI IN channel"></label><label>OUT channel<input id="midi-out-channel" data-setting="midi_out_channel" type="number" min="1" max="16" value="1" aria-label="MIDI OUT channel"></label></div><div class="setting-row"><span>MIDI OUT</span><div class="segments"><button id="midi-off" data-action="midi-mode" data-value="off">Off</button><button id="midi-out" data-action="midi-mode" data-value="out">Out</button><button id="midi-thru" data-action="midi-mode" data-value="thru">Thru</button></div></div><details><summary>MIDI CC</summary><div class="cc-grid">{cc}</div><button class="text-button" data-action="reset-cc">Reset CC</button></details></div></div>{pedal_settings}</section>
 <footer class="transport"><div class="source-controls"><button class="icon-button" data-action="open-audio" aria-label="Open audio file" title="Open audio file">{folder_icon}</button><button class="play-button" id="play-button" data-action="play" aria-label="Play" title="Play / Pause · Space">{play_icon}</button><div class="source-label"><strong id="source-name">No audio</strong><span id="source-duration">WAV · MP3 · FLAC · AIFF</span></div><button class="text-button" data-action="demo" id="demo-button">Demo</button></div><div class="transport-actions"><button id="loop-button" data-action="loop" class="loop-button" aria-pressed="true" title="Loop">↻</button><button id="mic-button" class="icon-button" data-action="mic" aria-label="Live input" title="Live input">{mic_icon}</button><button id="record-button" class="icon-button record-button" data-action="record" aria-label="Record output" title="Record output" aria-pressed="false"><span></span></button><button class="button export-wav" id="export-button" data-action="render" disabled>{download_icon}<span>WAV</span></button></div></footer>
 <div class="status-line" role="status"><span id="status-text">Ready</span><span class="local-note">Processed on your device</span></div>
 <input type="file" id="audio-file" accept="audio/*,.wav,.mp3,.flac,.aiff,.ogg,.m4a" hidden><input type="file" id="bank-file" accept=".json,.elb" hidden><div id="dialog-root"></div>
</div>"#,
        settings_icon = icon("settings"),
        save_icon = icon("save"),
        shuffle_icon = icon("shuffle"),
        prev_icon = icon("prev"),
        next_icon = icon("next"),
        pedal_settings = pedal_settings(s),
        compression = small_range(s, 12, "Compression"),
        dub_mix = small_range(s, 16, "Dub mix"),
        midi_status = if midi_supported() {
            "0 = Omni"
        } else {
            "Web MIDI is unavailable in this browser"
        },
        folder_icon = icon("folder"),
        play_icon = icon("play"),
        mic_icon = icon("mic"),
        download_icon = icon("download")
    )
}

fn render_list(s: &State) {
    let filter = s.search.to_lowercase();
    let rows=s.bank.presets.iter().enumerate().filter(|(_,p)|p.name.to_lowercase().contains(&filter)).map(|(i,p)|format!("<button class='preset-item' role='option' aria-selected='{}' data-action='preset' data-index='{i}'><span class='preset-index'>{:02}</span><span>{}</span><i>{}</i></button>",i==s.active,i+1,escape(&p.name),if p.factory{""}else{"U"})).collect::<String>();
    if let Some(e) = el("preset-list") {
        e.set_inner_html(if rows.is_empty() {
            "<div class='empty-presets'>No presets found</div>"
        } else {
            &rows
        });
    }
    text("preset-count", &format!("{} / 100", s.bank.presets.len()));
}
fn refresh(shared: &Shared) {
    let s = shared.borrow();
    text("patch-name", s.name());
    text("patch-number", &format!("{:02}", s.active + 1));
    attr(
        "dirty-indicator",
        "style",
        if s.dirty { "opacity:1" } else { "opacity:0" },
    );
    text("time-display", &s.value_text(3));
    text("tempo-display", &format!("{:.1} BPM", s.bpm));
    text(
        "sound-mode",
        if s.values[0] >= 0.5 { "BBD" } else { "TAPE" },
    );
    text(
        "sync-mode-label",
        match s.sync {
            1 => "TAP SYNC",
            2 => "MIDI SYNC",
            _ => "FREE",
        },
    );
    for id in 0..20 {
        let value = s.values[id];
        let value_text = s.value_text(id);
        text(&format!("value-{id}"), &value_text);
        if let Some(e) =
            el(&format!("parameter-{id}")).and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
        {
            e.set_value(&value.to_string());
            let _ = e.set_attribute("aria-valuetext", &value_text);
        }
        attr(
            &format!("arc-{id}"),
            "stroke-dasharray",
            &format!("{} 226.2", 169.65 * value),
        );
        attr(
            &format!("pointer-{id}"),
            "transform",
            &format!("rotate({} 44 44)", -135.0 + 270.0 * value),
        );
        text(&format!("toggle-{id}"), &value_text);
        pressed(&format!("toggle-{id}"), value >= 0.5);
    }
    pressed("freeze-button", s.values[19] >= 0.5);
    pressed("bypass-button", s.settings.bypass);
    for i in 0..3 {
        pressed(
            ["sync-free", "sync-tap", "sync-midi"][i],
            s.sync as usize == i,
        );
    }
    pressed("record-button", s.recording);
    pressed("mic-button", s.live && s.playing);
    attr(
        "record-button",
        "aria-label",
        if s.recording {
            "Stop recording"
        } else {
            "Record output"
        },
    );
    if let Some(e) = el("play-button") {
        e.set_inner_html(&icon(if s.playing { "pause" } else { "play" }));
        let _ = e.set_attribute("aria-label", if s.playing { "Pause" } else { "Play" });
    }
    text("source-name", &s.source);
    text(
        "source-duration",
        &if s.live {
            "LIVE INPUT".into()
        } else if s.duration > 0.0 {
            format!(
                "{}:{:02} · 48 kHz",
                s.duration as u64 / 60,
                s.duration as u64 % 60
            )
        } else {
            "WAV · MP3 · FLAC · AIFF".into()
        },
    );
    attr("audio-led", "class", if s.playing { "active" } else { "" });
    disabled("export-button", !s.loaded || s.live || s.busy);
    disabled("record-button", (!s.playing && !s.recording) || s.busy);
    disabled("play-button", s.busy || (!s.loaded && !s.live));
    disabled("midi-button", !midi_supported());
    text("status-text", &s.status);
    attr("status-text", "class", if s.error { "error" } else { "" });
    if let Some(e) = el("advanced-panel") {
        if s.advanced {
            let _ = e.remove_attribute("hidden");
        } else {
            let _ = e.set_attribute("hidden", "");
        }
    }
    attr(
        "settings-button",
        "aria-expanded",
        if s.advanced { "true" } else { "false" },
    );
    for (id, value) in [
        ("input-db", s.settings.input_db),
        ("output-db", s.settings.output_db),
    ] {
        if let Some(e) = el(id).and_then(|e| e.dyn_into::<HtmlInputElement>().ok()) {
            e.set_value(&value.to_string());
        }
    }
    text("input-db-value", &format!("{:+.0} dB", s.settings.input_db));
    text(
        "output-db-value",
        &format!("{:+.0} dB", s.settings.output_db),
    );
    for (id, value) in [
        ("midi-in-channel", s.settings.midi_channel),
        ("midi-out-channel", s.settings.midi_out_channel),
    ] {
        if let Some(e) = el(id).and_then(|e| e.dyn_into::<HtmlInputElement>().ok()) {
            e.set_value(&value.to_string());
        }
    }
    for (id, v) in [
        ("midi-off", !s.settings.midi_output && !s.settings.midi_thru),
        ("midi-out", s.settings.midi_output),
        ("midi-thru", s.settings.midi_thru),
    ] {
        pressed(id, v);
    }
    pressed("load-last", s.settings.load_last);
    text("load-last", if s.settings.load_last { "On" } else { "Off" });
}
fn select(shared: &Shared, index: usize) {
    shared.borrow_mut().select(index);
    render_list(&shared.borrow());
    refresh(shared);
}
fn save_dialog(shared: &Shared) {
    let s = shared.borrow();
    let name = escape(s.name());
    if let Some(root) = el("dialog-root") {
        root.set_inner_html(&format!(r#"<div class="dialog-backdrop"><section class="dialog" role="dialog" aria-modal="true" aria-labelledby="save-title"><h2 id="save-title">Save sound</h2><input id="save-name" aria-label="Preset name" maxlength="48" value="{name}"><div class="dialog-actions"><button class="button" data-action="cancel-dialog">Cancel</button><button class="button" data-action="overwrite-preset">Replace</button><button class="button primary" data-action="copy-preset">Save a copy</button></div></section></div>"#));
    }
    if let Some(e) = el("save-name").and_then(|e| e.dyn_into::<HtmlInputElement>().ok()) {
        let _ = e.focus();
        e.select();
    }
}
fn action(shared: &Shared, button: &Element) {
    if button.has_attribute("disabled") {
        return;
    }
    let action = button.get_attribute("data-action").unwrap_or_default();
    let index = || {
        button
            .get_attribute("data-index")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let id = || {
        button
            .get_attribute("data-id")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let number = || {
        button
            .get_attribute("data-value")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    match action.as_str() {
        "preset" => {
            select(shared, index());
            return;
        }
        "previous" | "next" => {
            let s = shared.borrow();
            let len = s.bank.presets.len();
            let i = (s.active + if action == "next" { 1 } else { len - 1 }) % len;
            drop(s);
            select(shared, i);
            return;
        }
        "save" => {
            save_dialog(shared);
            return;
        }
        "cancel-dialog" => {
            if let Some(e) = el("dialog-root") {
                e.set_inner_html("");
            }
            return;
        }
        "open-audio" | "import-bank" => {
            let key = if action == "open-audio" {
                "audio-file"
            } else {
                "bank-file"
            };
            if let Some(e) = el(key).and_then(|e| e.dyn_into::<HtmlInputElement>().ok()) {
                e.click();
            }
            return;
        }
        "demo" => {
            let data = keklocator_engine::wave::demo();
            let _ = audio_demo(&js_sys::Float32Array::from(data.as_slice()), 48000);
            return;
        }
        "play" => {
            send(json!({"type":"play"}));
            return;
        }
        "mic" => {
            send(json!({"type":"mic"}));
            return;
        }
        "midi" => {
            send(json!({"type":"midi"}));
            return;
        }
        "export-json" | "export-elb" => {
            let s = shared.borrow();
            let data = if action == "export-json" {
                Ok((
                    s.bank.to_json().into_bytes(),
                    "keklocator-bank.json",
                    "application/json",
                ))
            } else {
                s.bank
                    .to_elb()
                    .map(|v| (v, "keklocator-bank.elb", "application/octet-stream"))
            };
            drop(s);
            match data {
                Ok((bytes, name, mime)) => {
                    download(&js_sys::Uint8Array::from(bytes.as_slice()), name, mime)
                }
                Err(e) => {
                    shared.borrow_mut().status = e;
                    shared.borrow_mut().error = true;
                }
            }
        }
        "copy-preset" | "overwrite-preset" => {
            let name = el("save-name")
                .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                .map(|e| e.value().trim().to_owned())
                .unwrap_or_default();
            let mut s = shared.borrow_mut();
            let preset = Preset {
                name,
                values: s.values,
                factory: false,
            };
            if let Err(e) = preset.validate() {
                s.status = e;
                s.error = true;
            } else if action == "copy-preset" && s.bank.presets.len() >= 100 {
                s.status = "The bank is limited to 100 presets".into();
                s.error = true;
            } else {
                if action == "copy-preset" {
                    s.bank.presets.push(preset);
                    s.active = s.bank.presets.len() - 1;
                } else {
                    let i = s.active;
                    s.bank.presets[i] = preset;
                }
                s.dirty = false;
                s.status = "Preset saved".into();
                s.error = false;
                s.save();
                if let Some(e) = el("dialog-root") {
                    e.set_inner_html("");
                }
            }
            drop(s);
            render_list(&shared.borrow());
        }
        "magic" => {
            let mut s = shared.borrow_mut();
            s.values = magic(js_sys::Date::now() as u64, s.values);
            s.dirty = true;
            s.send_all(s.settings.morph_seconds);
            s.status = "Magic".into();
            s.error = false;
            s.save();
        }
        "toggle" | "freeze" => {
            let id = if action == "freeze" { 19 } else { id() };
            let mut s = shared.borrow_mut();
            let value = if s.values[id] >= 0.5 { 0.0 } else { 1.0 };
            s.set_parameter(id, value, true);
            s.save();
        }
        "bypass" => {
            let mut s = shared.borrow_mut();
            s.settings.bypass = !s.settings.bypass;
            s.send_host();
            s.save();
        }
        "tap" => shared.borrow_mut().tap(),
        "sync" => {
            let mut s = shared.borrow_mut();
            s.sync = (number() as u8).min(2);
            s.send_sync();
            s.save();
        }
        "advanced" => {
            let mut s = shared.borrow_mut();
            s.advanced = !s.advanced;
        }
        "morph" => {
            let mut s = shared.borrow_mut();
            s.settings.morph_seconds = number();
            s.save();
            if let Ok(nodes) = doc().query_selector_all("[data-action='morph']") {
                for i in 0..nodes.length() {
                    if let Some(e) = nodes.item(i).and_then(|e| e.dyn_into::<Element>().ok()) {
                        let _ = e.set_attribute(
                            "aria-pressed",
                            if e.get_attribute("data-value") == button.get_attribute("data-value") {
                                "true"
                            } else {
                                "false"
                            },
                        );
                    }
                }
            }
        }
        "load-last" => {
            let mut s = shared.borrow_mut();
            s.settings.load_last = !s.settings.load_last;
            s.save();
        }
        "clear" => {
            send(json!({"type":"clear"}));
            let mut s = shared.borrow_mut();
            s.status = "Buffer cleared".into();
            s.error = false;
        }
        "loop" => {
            let enabled = button.get_attribute("aria-pressed").as_deref() != Some("true");
            let _ = button.set_attribute("aria-pressed", if enabled { "true" } else { "false" });
            send(json!({"type":"loop","enabled":enabled}));
        }
        "record" => {
            let enabled = !shared.borrow().recording;
            send(json!({"type":"record","enabled":enabled}));
        }
        "render" => {
            let tail = el("tail-options")
                .and_then(|e| e.query_selector("[aria-pressed='true']").ok().flatten())
                .and_then(|e| e.get_attribute("data-value"))
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(4.0);
            send(json!({"type":"render","tail":tail}));
        }
        "tail" => {
            if let Some(p) = button.parent_element() {
                if let Ok(nodes) = p.query_selector_all("button") {
                    for i in 0..nodes.length() {
                        if let Some(e) = nodes.item(i).and_then(|e| e.dyn_into::<Element>().ok()) {
                            let _ = e.set_attribute(
                                "aria-pressed",
                                if e == *button { "true" } else { "false" },
                            );
                        }
                    }
                }
            }
        }
        "midi-mode" => {
            let value = button.get_attribute("data-value").unwrap_or_default();
            let mut s = shared.borrow_mut();
            s.settings.midi_output = value == "out";
            s.settings.midi_thru = value == "thru";
            s.save();
        }
        "reset-cc" => {
            let mut s = shared.borrow_mut();
            s.settings.midi_cc = std::array::from_fn(|i| 70 + i as u8);
            s.save();
            if let Ok(nodes) = doc().query_selector_all("[data-cc]") {
                for i in 0..nodes.length() {
                    if let Some(e) = nodes
                        .item(i)
                        .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                    {
                        if let Some(id) = e
                            .get_attribute("data-cc")
                            .and_then(|v| v.parse::<usize>().ok())
                        {
                            e.set_value(&s.settings.midi_cc[id].to_string());
                        }
                    }
                }
            }
        }
        "move-up" | "move-down" => {
            let mut s = shared.borrow_mut();
            let from = s.active;
            let to = if action == "move-up" {
                from.saturating_sub(1)
            } else {
                (from + 1).min(s.bank.presets.len() - 1)
            };
            s.bank.presets.swap(from, to);
            s.active = to;
            s.save();
            drop(s);
            render_list(&shared.borrow());
        }
        "delete" => {
            let mut s = shared.borrow_mut();
            if s.bank.presets.len() > 1 {
                let i = s.active;
                s.bank.presets.remove(i);
                let next = i.min(s.bank.presets.len() - 1);
                s.select(next);
            }
            drop(s);
            render_list(&shared.borrow());
        }
        _ => {}
    }
    refresh(shared);
}

fn listen(target: &web_sys::EventTarget, name: &str, callback: impl Fn(Event) + 'static) {
    // File inputs and downloads dispatch nested click events synchronously.
    let closure = Closure::<dyn Fn(Event)>::new(callback);
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}
fn input(shared: &Shared, e: &HtmlInputElement) {
    if e.id() == "preset-search" {
        shared.borrow_mut().search = e.value();
        render_list(&shared.borrow());
        return;
    }
    if let Some(id) = e
        .get_attribute("data-param")
        .and_then(|v| v.parse::<usize>().ok())
    {
        if let Ok(value) = e.value().parse::<f32>() {
            if id < 20 {
                shared.borrow_mut().set_parameter(id, value, true);
            }
        }
    }
    if let Some(key) = e.get_attribute("data-host") {
        if let Ok(value) = e.value().parse::<f32>() {
            let mut s = shared.borrow_mut();
            if key == "input_db" {
                s.settings.input_db = value.clamp(-24.0, 6.0);
            } else {
                s.settings.output_db = value.clamp(-12.0, 0.0);
            }
            s.send_host();
        }
    }
    if let Some(key) = e.get_attribute("data-setting") {
        if let Ok(value) = e.value().parse::<u8>() {
            let mut s = shared.borrow_mut();
            if key == "midi_channel" {
                s.settings.midi_channel = value.min(16);
            } else {
                s.settings.midi_out_channel = value.clamp(1, 16);
            }
        }
    }
    if let Some(id) = e
        .get_attribute("data-cc")
        .and_then(|v| v.parse::<usize>().ok())
    {
        if let Ok(value) = e.value().parse::<u8>() {
            if id < 13 {
                shared.borrow_mut().settings.midi_cc[id] = value.min(127);
            }
        }
    }
    refresh(shared);
}
fn on_audio(shared: &Shared, event: Event) {
    let Some(custom) = event.dyn_ref::<web_sys::CustomEvent>() else {
        return;
    };
    let Some(string) = custom.detail().as_string() else {
        return;
    };
    let Ok(data) = serde_json::from_str::<Value>(&string) else {
        return;
    };
    let mut s = shared.borrow_mut();
    match data["type"].as_str().unwrap_or("") {
        "status" => {
            s.status = data["message"].as_str().unwrap_or_default().into();
            s.busy = data["busy"].as_bool().unwrap_or(false);
            s.error = false;
        }
        "error" => {
            s.status = data["message"].as_str().unwrap_or("Audio error").into();
            s.error = true;
            s.busy = false;
            if data["fatal"].as_bool().unwrap_or(false) {
                s.playing = false;
                s.recording = false;
                s.live = false;
            }
        }
        "loaded" => {
            s.source = data["name"].as_str().unwrap_or("Audio file").into();
            s.duration = data["duration"].as_f64().unwrap_or(0.0);
            s.loaded = true;
            s.live = false;
            s.playing = data["playing"].as_bool().unwrap_or(false);
            s.status = "Audio loaded".into();
            s.busy = false;
            s.error = false;
        }
        "playback" => {
            s.playing = data["playing"].as_bool().unwrap_or(false);
            if let Some(name) = data["name"].as_str() {
                s.source = name.into();
            }
            s.live = data["live"].as_bool().unwrap_or(false);
            s.status = if s.playing { "Playing" } else { "Paused" }.into();
            s.error = false;
        }
        "recording" => {
            s.recording = data["recording"].as_bool().unwrap_or(false);
            s.busy = data["saving"].as_bool().unwrap_or(false);
            s.status = if s.recording {
                "Recording output…"
            } else if s.busy {
                "Saving recording…"
            } else {
                "Recording saved as WAV"
            }
            .into();
            s.error = false;
        }
        "render" => {
            s.busy = data["busy"].as_bool().unwrap_or(false);
            s.progress = data["progress"].as_f64().unwrap_or(0.0) as f32;
            s.status = if s.busy {
                format!("Rendering WAV · {:.0}%", s.progress * 100.0)
            } else {
                "WAV ready".into()
            };
            s.error = false;
        }
        "midi" => {
            s.midi_count = data["count"].as_u64().unwrap_or(0) as usize;
            text(
                "midi-status",
                &format!("Inputs: {} · 0 = Omni", s.midi_count),
            );
            s.status = "MIDI connected".into();
            s.error = false;
        }
        "meter" => {
            for (key, id) in [("left", "meter-l"), ("right", "meter-r")] {
                let level = data[key].as_f64().unwrap_or(0.0);
                let width = if level > 0.0 {
                    ((20.0 * level.log10() + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                attr(id, "style", &format!("width:{width}%"));
            }
            if let Some(array) = data["wave"].as_array() {
                for value in array {
                    if let (Some(l), Some(r)) = (value[0].as_f64(), value[1].as_f64()) {
                        s.wave.push_back([l as f32, r as f32]);
                    }
                }
            }
            while s.wave.len() > 180 {
                s.wave.pop_front();
            }
            for (channel, id, baseline) in [(0, "scope-l", 18.0), (1, "scope-r", 54.0)] {
                let path = s
                    .wave
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        format!(
                            "{}{} {:.2}",
                            if i == 0 { "M" } else { "L" },
                            i as f32 * 2.0,
                            baseline - v[channel].clamp(-1.0, 1.0) * 17.0
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                attr(id, "d", &path);
            }
            let progress = data["morph"].as_f64().unwrap_or(1.0);
            attr(
                "morph-progress",
                "style",
                &format!(
                    "width:{}%;opacity:{}",
                    progress * 100.0,
                    if progress < 1.0 { 1.0 } else { 0.0 }
                ),
            );
            if let Some(taps) = data["taps"].as_array() {
                let max = taps
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(0.001f64, f64::max);
                for (i, v) in taps.iter().enumerate() {
                    if let Some(v) = v.as_f64() {
                        attr(
                            &format!("tap-{i}"),
                            "cx",
                            &format!("{}", 8.0 + v / max * 344.0),
                        );
                    }
                }
            }
            return;
        }
        _ => {}
    }
    drop(s);
    refresh(shared);
}

#[wasm_bindgen]
pub fn mount() {
    let shared = Rc::new(RefCell::new(State::new()));
    if let Some(root) = el("app") {
        root.set_inner_html(&render(&shared.borrow()));
    }
    render_list(&shared.borrow());
    refresh(&shared);
    shared.borrow().send_all(0.0);
    let target: web_sys::EventTarget = doc().unchecked_into();
    {
        let shared = shared.clone();
        listen(&target, "click", move |e| {
            if let Some(element) = e.target().and_then(|v| v.dyn_into::<Element>().ok()) {
                if let Ok(Some(button)) = element.closest("[data-action]") {
                    action(&shared, &button);
                }
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "input", move |e| {
            if let Some(input) = e
                .target()
                .and_then(|v| v.dyn_into::<HtmlInputElement>().ok())
            {
                input_handler(&shared, &input);
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "dblclick", move |e| {
            if let Some(input) = e
                .target()
                .and_then(|v| v.dyn_into::<HtmlInputElement>().ok())
            {
                if let Some(id) = input
                    .get_attribute("data-param")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    if id < 20 {
                        shared.borrow_mut().set_parameter(
                            id,
                            PARAMETERS[id].normalized_default(),
                            true,
                        );
                        shared.borrow().save();
                        refresh(&shared);
                    }
                }
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "change", move |event| {
            if let Some(select) = event
                .target()
                .and_then(|v| v.dyn_into::<web_sys::HtmlSelectElement>().ok())
            {
                if let Some(key) = select.get_attribute("data-pedal") {
                    let value = select.value();
                    let mut s = shared.borrow_mut();
                    match key.as_str() {
                        "single" => s.settings.footswitch_single = value,
                        "double" => s.settings.footswitch_double = value,
                        "hold" => s.settings.footswitch_hold = value,
                        _ => {}
                    }
                    s.save();
                }
                return;
            }
            let Some(input) = event
                .target()
                .and_then(|v| v.dyn_into::<HtmlInputElement>().ok())
            else {
                return;
            };
            if input.id() == "audio-file" {
                if let Some(file) = input.files().and_then(|v| v.get(0)) {
                    let _ = audio_file(&file);
                }
                input.set_value("");
            } else if input.id() == "bank-file" {
                if let Some(file) = input.files().and_then(|v| v.get(0)) {
                    let shared = shared.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let result =
                            wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                        let result = result.map_err(|e| format!("{e:?}")).and_then(|v| {
                            let bytes = js_sys::Uint8Array::new(&v).to_vec();
                            if bytes.starts_with(&[0xef, 0xbf, 0x13, 0x1a]) {
                                PresetBank::from_elb(&bytes)
                            } else {
                                std::str::from_utf8(&bytes)
                                    .map_err(|e| e.to_string())
                                    .and_then(PresetBank::from_json)
                            }
                        });
                        let mut s = shared.borrow_mut();
                        match result {
                            Ok(bank) => {
                                s.bank = bank;
                                s.select(0);
                                s.status = format!("Presets loaded: {}", s.bank.presets.len());
                                s.error = false;
                            }
                            Err(e) => {
                                s.status = e;
                                s.error = true;
                            }
                        }
                        drop(s);
                        render_list(&shared.borrow());
                        refresh(&shared);
                    });
                }
                input.set_value("");
            } else {
                shared.borrow().save();
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "keydown", move |event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if key.repeat() || key.ctrl_key() || key.meta_key() || key.alt_key() {
                return;
            }
            if key.key() == "Escape" {
                if let Some(root) = el("dialog-root") {
                    root.set_inner_html("");
                }
                return;
            }
            if event
                .target()
                .and_then(|v| v.dyn_into::<Element>().ok())
                .is_some_and(|e| matches!(e.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
            {
                return;
            }
            if key.key().eq_ignore_ascii_case("p") {
                key.prevent_default();
                pedal_input(&shared, true);
                return;
            }
            let action = match key.key().to_lowercase().as_str() {
                " " => "play",
                "f" => "freeze",
                "b" => "bypass",
                "t" => "tap",
                "m" => "magic",
                "arrowleft" => "previous",
                "arrowright" => "next",
                _ => return,
            };
            key.prevent_default();
            if let Ok(Some(button)) = doc().query_selector(&format!("[data-action='{action}']")) {
                action_handler(&shared, &button);
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "keyup", move |event| {
            if event
                .dyn_ref::<KeyboardEvent>()
                .is_some_and(|key| key.key().eq_ignore_ascii_case("p"))
            {
                pedal_input(&shared, false);
            }
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "pointerdown", move |event| {
            if event
                .target()
                .and_then(|v| v.dyn_into::<Element>().ok())
                .is_some_and(|e| e.id() == "pedal-button")
            {
                pedal_input(&shared, true);
            }
        });
    }
    for event in ["pointerup", "pointercancel"] {
        let shared = shared.clone();
        listen(&target, event, move |_| pedal_input(&shared, false));
    }
    {
        let shared = shared.clone();
        let callback = Closure::<dyn FnMut()>::new(move || {
            let event = shared.borrow_mut().pedal.tick(now());
            if let Some(event) = event {
                pedal_event(&shared, event);
            }
        });
        let _ = window().set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            25,
        );
        callback.forget();
    }
    let target: web_sys::EventTarget = window().unchecked_into();
    {
        let shared = shared.clone();
        listen(&target, "blur", move |_| pedal_input(&shared, false));
    }
    {
        let shared = shared.clone();
        listen(&target, "erika-audio", move |e| on_audio(&shared, e));
    }
    {
        let shared = shared.clone();
        listen(&target, "erika-midi", move |event| {
            let Some(custom) = event.dyn_ref::<web_sys::CustomEvent>() else {
                return;
            };
            let Some(text) = custom.detail().as_string() else {
                return;
            };
            let Ok(data) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            let bytes: Vec<u8> = data["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().map(|v| v as u8))
                        .collect()
                })
                .unwrap_or_default();
            let time = data["time"].as_f64().unwrap_or(0.0);
            let mut s = shared.borrow_mut();
            if s.settings.midi_thru {
                send(json!({"type":"midi_out","data":bytes}));
            }
            let channel = s.settings.midi_channel;
            let map = s.settings.midi_cc;
            if let Some(action) = s.midi.message(&bytes, time, channel, &map) {
                match action {
                    MidiAction::Parameter(id, value) => s.set_parameter(id, value, false),
                    MidiAction::Program(i) => s.select(i),
                    MidiAction::Tempo(bpm) if s.sync == 2 => {
                        if (20.0..=400.0).contains(&bpm) {
                            s.bpm = bpm;
                            s.send_sync();
                        }
                    }
                    MidiAction::Note(note) => {
                        s.set_parameter(3, keklocator_engine::midi::note_time(note), false);
                    }
                    _ => {}
                }
            }
            drop(s);
            render_list(&shared.borrow());
            refresh(&shared);
        });
    }
    {
        let shared = shared.clone();
        listen(&target, "pagehide", move |_| shared.borrow().save());
    }
}
fn input_handler(s: &Shared, e: &HtmlInputElement) {
    input(s, e)
}
fn action_handler(s: &Shared, e: &Element) {
    action(s, e)
}

fn pedal_event(shared: &Shared, event: PedalEvent) {
    if event == PedalEvent::HoldStart {
        let mut s = shared.borrow_mut();
        let key = s.settings.footswitch_hold.clone();
        if key == "freeze" {
            let previous = s.values[19] >= 0.5;
            s.pedal_restore = Some((key, previous));
            s.set_parameter(19, 1.0, true);
        } else if key == "bypass" {
            let previous = s.settings.bypass;
            s.pedal_restore = Some((key, previous));
            s.settings.bypass = true;
            s.send_host();
        }
        drop(s);
        refresh(shared);
        return;
    }
    if event == PedalEvent::HoldEnd {
        let mut s = shared.borrow_mut();
        if let Some((key, value)) = s.pedal_restore.take() {
            if key == "freeze" {
                s.set_parameter(19, if value { 1.0 } else { 0.0 }, true);
            } else {
                s.settings.bypass = value;
                s.send_host();
            }
        }
        drop(s);
        refresh(shared);
        return;
    }
    let key = match event {
        PedalEvent::Single => shared.borrow().settings.footswitch_single.clone(),
        PedalEvent::Double => shared.borrow().settings.footswitch_double.clone(),
        PedalEvent::Tap => "tap".into(),
        _ => return,
    };
    if key == "random" {
        let len = shared.borrow().bank.presets.len();
        select(shared, (js_sys::Math::random() * len as f64) as usize);
    } else if ["freeze", "bypass", "next", "previous", "magic", "tap"].contains(&key.as_str()) {
        if let Ok(Some(button)) = doc().query_selector(&format!("[data-action='{key}']")) {
            action(shared, &button);
        }
    }
}
fn pedal_input(shared: &Shared, down: bool) {
    let mut s = shared.borrow_mut();
    let tap_mode = s.settings.footswitch_single == "tap";
    let due = s.pedal.tick(now());
    let event = if down {
        s.pedal.press(now(), tap_mode)
    } else {
        s.pedal.release(now())
    };
    drop(s);
    if let Some(event) = due {
        pedal_event(shared, event);
    }
    if let Some(event) = event {
        pedal_event(shared, event);
    }
    pressed("pedal-button", down);
}
