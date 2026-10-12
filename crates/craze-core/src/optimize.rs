//! Width optimization: scales all conductor path widths until the simulated resistance
//! matches a target value.
//!
//! R depends on the width scale s roughly like 1/s, so a secant iteration on
//! ln R(ln s) converges in a few steps.

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

use serde::Serialize;

use crate::error::{CrazeError, Result};
use crate::gds::{Library, LayerKey};
use crate::setup::{SimSettings, Setup};
use crate::simulate::{ProgressFn, simulate};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeStep {
    pub iteration: u32,
    pub scale: f64,
    pub resistance: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeResult {
    pub steps: Vec<OptimizeStep>,
    pub best_scale: f64,
    pub best_resistance: f64,
    pub converged: bool,
}

pub struct OptimizeParams<'a> {
    pub library: &'a Library,
    pub top_cell: &'a str,
    pub setup: &'a Setup,
    pub settings: &'a SimSettings,
    /// Sheet resistance in Ω/□ at the evaluation temperature.
    pub sheet_resistance: f64,
    pub target_ohm: f64,
    /// Relative tolerance, e.g. 0.002 for 0.2 %.
    pub tolerance: f64,
    pub max_iterations: u32,
}

pub fn optimize(
    p: &OptimizeParams,
    cancel: &AtomicBool,
    progress: ProgressFn,
    on_step: &(dyn Fn(&OptimizeStep) + Sync),
) -> Result<OptimizeResult> {
    let keys: HashSet<LayerKey> = p.setup.conductor_keys().into_iter().collect();
    if p.library.count_paths(&keys) == 0 {
        return Err(CrazeError::Setup(
            "Die Optimierung verändert Pfadbreiten, aber auf den Leiter-Layern gibt es keine Pfade (nur Polygone).".into(),
        ));
    }
    if !(p.target_ohm > 0.0) || !(p.sheet_resistance > 0.0) {
        return Err(CrazeError::Setup("Zielwiderstand und Flächenwiderstand müssen positiv sein.".into()));
    }

    let eval = |s: f64| -> Result<f64> {
        let recs = p.library.with_scaled_path_widths(&keys, s);
        let lib = Library::from_records(recs)?;
        let flat = lib.flatten(p.top_cell)?;
        let out = simulate(&flat, p.setup, p.settings, cancel, progress)?;
        Ok(out.summary.squares * p.sheet_resistance)
    };

    let mut steps: Vec<OptimizeStep> = Vec::new();
    let push = |steps: &mut Vec<OptimizeStep>, s: f64, r: f64| {
        let st = OptimizeStep { iteration: steps.len() as u32, scale: s, resistance: r };
        on_step(&st);
        steps.push(st);
    };

    let mut s0 = 1.0;
    let mut r0 = eval(s0)?;
    push(&mut steps, s0, r0);
    let ok = |r: f64| ((r - p.target_ohm) / p.target_ohm).abs() <= p.tolerance;
    // First guess: R ∝ 1/s
    let mut s1 = (s0 * r0 / p.target_ohm).clamp(0.05, 20.0);
    let mut converged = ok(r0);
    while !converged && (steps.len() as u32) < p.max_iterations {
        let r1 = eval(s1)?;
        push(&mut steps, s1, r1);
        if ok(r1) {
            converged = true;
            break;
        }
        // Secant step in log-log space.
        let (l0, l1) = (s0.ln(), s1.ln());
        let (f0, f1) = (r0.ln() - p.target_ohm.ln(), r1.ln() - p.target_ohm.ln());
        let slope = if (l1 - l0).abs() > 1e-12 { (f1 - f0) / (l1 - l0) } else { -1.0 };
        let slope = if slope < -0.05 { slope } else { -1.0 };
        let next = (l1 - f1 / slope).exp().clamp(s1 * 0.25, s1 * 4.0).clamp(0.05, 20.0);
        s0 = s1;
        r0 = r1;
        s1 = next;
    }
    let best = steps
        .iter()
        .min_by(|a, b| (a.resistance - p.target_ohm).abs().total_cmp(&(b.resistance - p.target_ohm).abs()))
        .unwrap();
    Ok(OptimizeResult { best_scale: best.scale, best_resistance: best.resistance, converged, steps })
}
