#[cfg(any(target_arch = "wasm32", test))]
mod plugin_model;
#[cfg(target_arch = "wasm32")]
mod plugin_ui;
#[cfg(target_arch = "wasm32")]
mod ui;
