//! Tauri command layer. All logic lives in [`backend`]; large numeric data is returned as
//! raw little-endian buffers so the frontend can read it as typed arrays.

pub mod backend;

use std::path::PathBuf;

use backend::{Backend, CmdResult, Example, LayoutInfo, OptimizeArgs, StudyPoint};
use base64::Engine;
use craze_core::gds::LayerKey;
use craze_core::materials::Material;
use craze_core::optimize::{OptimizeResult, OptimizeStep};
use craze_core::setup::{SimSettings, Setup};
use craze_core::simulate::{Progress, Summary};
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Manager, State};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

fn backend(app: &AppHandle) -> &Backend {
    app.state::<Backend>().inner()
}

#[tauri::command]
async fn open_layout(app: AppHandle, path: String, top_cell: Option<String>) -> CmdResult<LayoutInfo> {
    blocking(move || backend(&app).open_layout(&path, top_cell)).await
}

#[tauri::command]
fn layout_geometry(state: State<'_, Backend>) -> CmdResult<Response> {
    Ok(Response::new(state.layout_geometry()?))
}

#[tauri::command]
async fn run_simulation(
    app: AppHandle,
    setup: Setup,
    settings: SimSettings,
    on_progress: Channel<Progress>,
) -> CmdResult<Summary> {
    blocking(move || {
        backend(&app).simulate(&setup, &settings, &|p| {
            let _ = on_progress.send(p);
        })
    })
    .await
}

#[tauri::command]
fn result_fields(state: State<'_, Backend>) -> CmdResult<Response> {
    Ok(Response::new(state.result_fields()?))
}

#[tauri::command]
fn cancel_job(state: State<'_, Backend>) {
    state.cancel();
}

#[tauri::command]
async fn resolution_study(
    app: AppHandle,
    setup: Setup,
    settings: SimSettings,
    resolutions: Vec<u32>,
    on_point: Channel<StudyPoint>,
) -> CmdResult<Vec<StudyPoint>> {
    blocking(move || {
        backend(&app).resolution_study(&setup, &settings, &resolutions, &|p| {
            let _ = on_point.send(p.clone());
        })
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_optimization(
    app: AppHandle,
    setup: Setup,
    settings: SimSettings,
    sheet_resistance: f64,
    target_ohm: f64,
    tolerance: f64,
    max_iterations: u32,
    on_step: Channel<OptimizeStep>,
    on_progress: Channel<Progress>,
) -> CmdResult<OptimizeResult> {
    let args = OptimizeArgs { setup, settings, sheet_resistance, target_ohm, tolerance, max_iterations };
    blocking(move || {
        backend(&app).optimize(
            &args,
            &|s| {
                let _ = on_step.send(s.clone());
            },
            &|p| {
                let _ = on_progress.send(p);
            },
        )
    })
    .await
}

#[tauri::command]
fn save_optimized_layout(path: String, scale: f64, conductor_keys: Vec<LayerKey>, state: State<'_, Backend>) -> CmdResult<()> {
    state.save_optimized(&path, scale, conductor_keys)
}

#[tauri::command]
fn material_library() -> Vec<Material> {
    backend::material_library()
}

#[tauri::command]
fn write_text_file(path: String, contents: String) -> CmdResult<()> {
    std::fs::write(&path, contents).map_err(|e| format!("Saving failed: {e}"))
}

#[tauri::command]
fn read_text_file(path: String) -> CmdResult<String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Cannot read the file: {e}"))
}

/// Writes base64 encoded data (e.g. a PNG data URL).
#[tauri::command]
fn write_base64_file(path: String, data: String) -> CmdResult<()> {
    let payload = data.split_once(',').map(|(_, d)| d).unwrap_or(&data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(payload).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("Saving failed: {e}"))
}

fn prefs_path(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("prefs.json"))
}

#[tauri::command]
fn load_prefs(app: AppHandle) -> serde_json::Value {
    prefs_path(&app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
fn save_prefs(app: AppHandle, prefs: serde_json::Value) -> CmdResult<()> {
    let p = prefs_path(&app)?;
    std::fs::write(p, serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_examples(app: AppHandle) -> Vec<Example> {
    let bundled = app.path().resource_dir().ok().map(|d| d.join("examples")).filter(|d| d.is_dir());
    backend::list_examples(bundled.or_else(backend::dev_examples_dir))
}

/// File passed on the command line (e.g. "Open with" or `craze-app layout.gds`).
#[tauri::command]
fn startup_file() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).is_file())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Backend::default())
        .invoke_handler(tauri::generate_handler![
            open_layout,
            layout_geometry,
            run_simulation,
            result_fields,
            cancel_job,
            resolution_study,
            run_optimization,
            save_optimized_layout,
            material_library,
            write_text_file,
            read_text_file,
            write_base64_file,
            load_prefs,
            save_prefs,
            list_examples,
            startup_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CRAZE");
}
