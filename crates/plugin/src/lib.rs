//! C ABI for the small DAW format/WebView shell. A handle belongs to one audio thread.
use keklocator_engine::daw::{self, DawProcessor, PARAM_COUNT};
#[no_mangle]
pub extern "C" fn ec_daw_note_time(note: u32) -> f32 {
    keklocator_engine::midi::note_time(note.min(127) as u8)
}
#[no_mangle]
pub extern "C" fn ec_daw_cc_value(value: u32) -> f32 {
    value.min(127) as f32 * 0.007874016
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_program(
    handle: *mut DawProcessor,
    index: u32,
    output: *mut f32,
) -> i32 {
    let Some(v) = (unsafe { handle.as_mut() }) else {
        return -1;
    };
    match v.program(index as usize) {
        Ok(true) => {
            if !output.is_null() {
                unsafe { std::slice::from_raw_parts_mut(output, 20) }
                    .copy_from_slice(&v.parameters[..20]);
            }
            0
        }
        Ok(false) => 1,
        Err(_) => -1,
    }
}
#[no_mangle]
pub extern "C" fn ec_daw_create(rate: f64) -> *mut DawProcessor {
    DawProcessor::new(rate).map_or(std::ptr::null_mut(), |v| Box::into_raw(Box::new(v)))
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_destroy(handle: *mut DawProcessor) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_set(handle: *mut DawProcessor, id: u32, value: f32) -> i32 {
    unsafe { handle.as_mut() }.map_or(-1, |v| {
        v.set_parameter(id as usize, value).map_or(-1, |_| 0)
    })
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_preset(
    handle: *mut DawProcessor,
    values: *const f32,
    seconds: f32,
) -> i32 {
    if values.is_null() {
        return -1;
    }
    unsafe { handle.as_mut() }.map_or(-1, |v| {
        v.preset(
            unsafe { std::slice::from_raw_parts(values, 20) }
                .try_into()
                .unwrap(),
            seconds,
        )
        .map_or(-1, |_| 0)
    })
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_process(
    handle: *mut DawProcessor,
    audio: *mut f32,
    frames: u32,
    tempo: f64,
    beat: f64,
    playing: u32,
) -> i32 {
    if audio.is_null() || frames > 1048576 {
        return -1;
    }
    unsafe { handle.as_mut() }.map_or(-1, |v| {
        v.process(
            unsafe { std::slice::from_raw_parts_mut(audio.cast::<[f32; 2]>(), frames as usize) },
            tempo,
            beat,
            playing != 0,
        )
        .map_or(-1, |_| 0)
    })
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_latency(handle: *const DawProcessor) -> u32 {
    unsafe { handle.as_ref() }.map_or(0, |v| v.latency())
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_step(handle: *const DawProcessor) -> i32 {
    unsafe { handle.as_ref() }.map_or(-1, |v| v.step)
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_tempo(handle: *const DawProcessor) -> f64 {
    unsafe { handle.as_ref() }.map_or(120., |v| v.tempo)
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_defaults(output: *mut f32) {
    if !output.is_null() {
        unsafe { std::slice::from_raw_parts_mut(output, PARAM_COUNT) }
            .copy_from_slice(&daw::defaults());
    }
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_name(id: u32, output: *mut u8, capacity: u32) {
    if output.is_null() || capacity == 0 {
        return;
    }
    let name = daw::name(id as usize);
    let n = name.len().min(capacity as usize - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(name.as_ptr(), output, n);
        *output.add(n) = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ec_daw_duck(handle: *const DawProcessor) -> f32 {
    unsafe { handle.as_ref() }.map_or(0., |v| v.duck_reduction)
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_display(
    id: u32,
    value: f32,
    parameters: *const f32,
    tempo: f64,
    output: *mut u8,
    capacity: u32,
) {
    if parameters.is_null() || output.is_null() || capacity == 0 {
        return;
    }
    let p: &[f32; PARAM_COUNT] = unsafe { std::slice::from_raw_parts(parameters, PARAM_COUNT) }
        .try_into()
        .unwrap();
    let text = keklocator_engine::extended::display(id as usize, value, p, tempo);
    let mut n = text.len().min(capacity as usize - 1);
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(text.as_ptr(), output, n);
        *output.add(n) = 0;
    }
}
fn library_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::path::PathBuf::from(std::env::var_os("APPDATA")?);
    #[cfg(target_os = "macos")]
    let base =
        std::path::PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })?;
    Some(base.join("KekLocator/library.json"))
}
fn legacy_library_path() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?,
    );
    #[cfg(target_os = "macos")]
    let path = home.join("Library/Application Support/Echolocator Rust/library.json");
    #[cfg(not(target_os = "macos"))]
    let path = home.join(".echolocator-rust/library.json");
    Some(path)
}
#[no_mangle]
pub unsafe extern "C" fn ec_library_read(output: *mut u8, capacity: u32) -> i32 {
    let Some(path) = library_path() else {
        return -1;
    };
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match legacy_library_path().map(std::fs::read) {
                Some(Ok(data)) => data,
                Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => return -1,
                _ => b"{\"patches\":[],\"favorites\":[]}".to_vec(),
            }
        }
        Err(_) => return -1,
    };
    if data.len() > 1024 * 1024 || serde_json::from_slice::<serde_json::Value>(&data).is_err() {
        return -1;
    }
    if output.is_null() || capacity as usize <= data.len() {
        return data.len() as i32 + 1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), output, data.len());
        *output.add(data.len()) = 0;
    }
    data.len() as i32
}
#[no_mangle]
pub unsafe extern "C" fn ec_library_write(input: *const u8, length: u32) -> i32 {
    if input.is_null() || length > 1024 * 1024 {
        return -1;
    }
    let data = unsafe { std::slice::from_raw_parts(input, length as usize) };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return -1;
    };
    if !value["patches"].is_array() || !value["favorites"].is_array() {
        return -1;
    }
    let Some(path) = library_path() else {
        return -1;
    };
    let result = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        // Unique staging file makes concurrent editors safe; rename is atomic.
        use std::io::Write;
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp = path.with_extension(format!("{}.{}.tmp", std::process::id(), suffix));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(data)?;
            file.sync_all()?;
            std::fs::rename(&temp, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    })();
    if result.is_ok() {
        0
    } else {
        -1
    }
}
#[no_mangle]
pub unsafe extern "C" fn ec_daw_parse(
    id: u32,
    text: *const std::ffi::c_char,
    parameters: *const f32,
    output: *mut f32,
) -> i32 {
    if text.is_null() || parameters.is_null() || output.is_null() {
        return -1;
    }
    let Ok(text) = unsafe { std::ffi::CStr::from_ptr(text) }.to_str() else {
        return -1;
    };
    let p: &[f32; PARAM_COUNT] = unsafe { std::slice::from_raw_parts(parameters, PARAM_COUNT) }
        .try_into()
        .unwrap();
    if let Some(v) = keklocator_engine::extended::parse(id as usize, text, p) {
        unsafe { *output = v };
        0
    } else {
        -1
    }
}
