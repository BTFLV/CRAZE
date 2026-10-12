//! Application backend independent of Tauri: holds the loaded layout and the last result
//! and implements all operations. The Tauri commands (and the development HTTP bridge)
//! are thin wrappers around this type.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use craze_core::gds::{FlatLayout, LayerKey, Library, records};
use craze_core::geometry::{BBox, signed_area};
use craze_core::materials::{self, Material};
use craze_core::optimize::{OptimizeParams, OptimizeResult, OptimizeStep, optimize};
use craze_core::setup::{SimSettings, Setup};
use craze_core::simulate::{Progress, SimOutput, Summary, simulate};
use serde::{Deserialize, Serialize};

pub type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub struct Loaded {
    pub path: PathBuf,
    pub library: Library,
    pub top: String,
    pub flat: FlatLayout,
}

#[derive(Default)]
pub struct Backend {
    loaded: Mutex<Option<Arc<Loaded>>>,
    result: Mutex<Option<Arc<SimOutput>>>,
    cancel: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerInfo {
    key: LayerKey,
    shapes: usize,
    paths: usize,
    area_um2: f64,
    bbox: BBox,
    min_path_width_um: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelInfo {
    key: LayerKey,
    text: String,
    x: f64,
    y: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutInfo {
    path: String,
    file_name: String,
    db_unit_m: f64,
    top_cells: Vec<String>,
    top_cell: String,
    layers: Vec<LayerInfo>,
    bbox: BBox,
    labels: Vec<LabelInfo>,
    suggestion: Setup,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudyPoint {
    pub resolution: u32,
    pub squares: f64,
    pub unknowns: usize,
    pub time_ms: f64,
}

#[derive(Serialize)]
pub struct Example {
    name: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeArgs {
    pub setup: Setup,
    pub settings: SimSettings,
    pub sheet_resistance: f64,
    pub target_ohm: f64,
    pub tolerance: f64,
    pub max_iterations: u32,
}

/// Resets the busy flag when a job ends (also on panic or error).
pub struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn layout_info(l: &Loaded) -> LayoutInfo {
    let mut bbox = BBox::empty();
    let layers = l
        .flat
        .shapes
        .iter()
        .map(|(key, shapes)| {
            let mut b = BBox::empty();
            for s in shapes {
                b.union(&BBox::of(&s.polygon));
            }
            bbox.union(&b);
            LayerInfo {
                key: *key,
                shapes: shapes.len(),
                paths: shapes.iter().filter(|s| s.path_width > 0.0).count(),
                area_um2: shapes.iter().map(|s| signed_area(&s.polygon).abs()).sum(),
                bbox: b,
                min_path_width_um: shapes
                    .iter()
                    .filter(|s| s.path_width > 0.0)
                    .map(|s| s.path_width)
                    .min_by(|a, b| a.total_cmp(b)),
            }
        })
        .collect();
    LayoutInfo {
        path: l.path.to_string_lossy().into_owned(),
        file_name: l.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        db_unit_m: l.library.db_unit_m,
        top_cells: l.library.top_cells(),
        top_cell: l.top.clone(),
        layers,
        bbox,
        labels: l
            .flat
            .labels
            .iter()
            .map(|lb| LabelInfo { key: lb.key, text: lb.text.clone(), x: lb.pos.x, y: lb.pos.y })
            .collect(),
        suggestion: Setup::suggest(&l.flat),
    }
}

/// Picks the top cell with the most shapes after flattening.
fn best_top(lib: &Library) -> CmdResult<(String, FlatLayout)> {
    let mut best: Option<(String, FlatLayout, usize)> = None;
    for top in lib.top_cells() {
        let flat = lib.flatten(&top).map_err(err)?;
        let n: usize = flat.shapes.values().map(|v| v.len()).sum();
        if best.as_ref().is_none_or(|b| n > b.2) {
            best = Some((top, flat, n));
        }
    }
    best.map(|(t, f, _)| (t, f)).ok_or_else(|| "Die Datei enthält keine Zellen.".to_string())
}

impl Backend {
    fn loaded(&self) -> CmdResult<Arc<Loaded>> {
        self.loaded.lock().unwrap().clone().ok_or_else(|| "Es ist kein Layout geladen.".to_string())
    }

    fn start_job(&self) -> CmdResult<BusyGuard> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err("Es läuft bereits eine Berechnung.".into());
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(BusyGuard(self.busy.clone()))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn open_layout(&self, path: &str, top_cell: Option<String>) -> CmdResult<LayoutInfo> {
        let p = PathBuf::from(path);
        let bytes = std::fs::read(&p).map_err(|e| format!("Datei kann nicht gelesen werden: {e}"))?;
        let library = Library::from_bytes(&bytes).map_err(err)?;
        let (top, flat) = match top_cell {
            Some(t) if library.structures.iter().any(|s| s.name == t) => {
                let f = library.flatten(&t).map_err(err)?;
                (t, f)
            }
            _ => best_top(&library)?,
        };
        if flat.shapes.is_empty() {
            return Err("Die Zelle enthält keine Geometrie.".into());
        }
        let loaded = Loaded { path: p, library, top, flat };
        let info = layout_info(&loaded);
        *self.loaded.lock().unwrap() = Some(Arc::new(loaded));
        *self.result.lock().unwrap() = None;
        Ok(info)
    }

    /// Polygons of all layers for the preview:
    /// f64 cx, f64 cy, u32 layers; per layer: i32 layer, i32 datatype, u32 polys;
    /// per poly: u32 n, n×(f32 x, f32 y) relative to (cx, cy).
    pub fn layout_geometry(&self) -> CmdResult<Vec<u8>> {
        let l = self.loaded()?;
        let mut bbox = BBox::empty();
        for shapes in l.flat.shapes.values() {
            for s in shapes {
                bbox.union(&BBox::of(&s.polygon));
            }
        }
        let (cx, cy) = ((bbox.min.x + bbox.max.x) * 0.5, (bbox.min.y + bbox.max.y) * 0.5);
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&cx.to_le_bytes());
        out.extend_from_slice(&cy.to_le_bytes());
        out.extend_from_slice(&(l.flat.shapes.len() as u32).to_le_bytes());
        for (key, shapes) in &l.flat.shapes {
            out.extend_from_slice(&(key.layer as i32).to_le_bytes());
            out.extend_from_slice(&(key.datatype as i32).to_le_bytes());
            out.extend_from_slice(&(shapes.len() as u32).to_le_bytes());
            for s in shapes {
                out.extend_from_slice(&(s.polygon.len() as u32).to_le_bytes());
                for p in &s.polygon {
                    out.extend_from_slice(&((p.x - cx) as f32).to_le_bytes());
                    out.extend_from_slice(&((p.y - cy) as f32).to_le_bytes());
                }
            }
        }
        Ok(out)
    }

    pub fn simulate(&self, setup: &Setup, settings: &SimSettings, progress: &(dyn Fn(Progress) + Sync)) -> CmdResult<Summary> {
        let _g = self.start_job()?;
        let l = self.loaded()?;
        let out = simulate(&l.flat, setup, settings, &self.cancel, progress).map_err(err)?;
        let summary = out.summary.clone();
        *self.result.lock().unwrap() = Some(Arc::new(out));
        Ok(summary)
    }

    /// u32 nx, u32 ny, then potential, jx, jy, coverage (f32 each), material (u8).
    pub fn result_fields(&self) -> CmdResult<Vec<u8>> {
        let r = self.result.lock().unwrap().clone().ok_or("Keine Ergebnisse vorhanden.")?;
        let f = &r.fields;
        let g = &r.summary.grid;
        let n = g.nx * g.ny;
        let mut out = Vec::with_capacity(8 + 17 * n);
        out.extend_from_slice(&(g.nx as u32).to_le_bytes());
        out.extend_from_slice(&(g.ny as u32).to_le_bytes());
        for buf in [&f.potential, &f.jx, &f.jy, &f.coverage] {
            for v in buf.iter() {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&f.material);
        Ok(out)
    }

    pub fn resolution_study(
        &self,
        setup: &Setup,
        settings: &SimSettings,
        resolutions: &[u32],
        on_point: &dyn Fn(&StudyPoint),
    ) -> CmdResult<Vec<StudyPoint>> {
        let _g = self.start_job()?;
        let l = self.loaded()?;
        let mut pts = Vec::new();
        for &r in resolutions {
            let s = SimSettings { resolution: r, ..settings.clone() };
            let out = simulate(&l.flat, setup, &s, &self.cancel, &|_| {}).map_err(err)?;
            let st = &out.summary.stats;
            let p = StudyPoint {
                resolution: r,
                squares: out.summary.squares,
                unknowns: st.unknowns,
                time_ms: st.time_raster_ms + st.time_build_ms + st.time_solve_ms + st.time_post_ms,
            };
            on_point(&p);
            pts.push(p);
        }
        Ok(pts)
    }

    pub fn optimize(
        &self,
        a: &OptimizeArgs,
        on_step: &(dyn Fn(&OptimizeStep) + Sync),
        progress: &(dyn Fn(Progress) + Sync),
    ) -> CmdResult<OptimizeResult> {
        let _g = self.start_job()?;
        let l = self.loaded()?;
        let p = OptimizeParams {
            library: &l.library,
            top_cell: &l.top,
            setup: &a.setup,
            settings: &a.settings,
            sheet_resistance: a.sheet_resistance,
            target_ohm: a.target_ohm,
            tolerance: a.tolerance,
            max_iterations: a.max_iterations,
        };
        optimize(&p, &self.cancel, progress, on_step).map_err(err)
    }

    pub fn save_optimized(&self, path: &str, scale: f64, conductor_keys: Vec<LayerKey>) -> CmdResult<()> {
        let l = self.loaded()?;
        let keys: HashSet<LayerKey> = conductor_keys.into_iter().collect();
        let recs = l.library.with_scaled_path_widths(&keys, scale);
        std::fs::write(path, records::write_binary(&recs)).map_err(|e| format!("Speichern fehlgeschlagen: {e}"))
    }
}

pub fn material_library() -> Vec<Material> {
    materials::library()
}

pub fn list_examples(dir: Option<PathBuf>) -> Vec<Example> {
    let Some(dir) = dir else { return vec![] };
    let mut v: Vec<Example> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "gds" || e == "gds2" || e == "gdsii"))
        .map(|p| Example {
            name: p.file_stem().unwrap_or_default().to_string_lossy().replace('_', " "),
            path: p.to_string_lossy().into_owned(),
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// Example directory of a source checkout (used in development).
pub fn dev_examples_dir() -> Option<PathBuf> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    dev.is_dir().then_some(dev)
}
