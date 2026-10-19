//! Complete simulation pipeline: layout → raster → network → solution → fields.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rayon::prelude::*;
use serde::Serialize;

use crate::error::{CrazeError, Result};
use crate::gds::FlatLayout;
use crate::network::{DIRS, Network, Neighbor};
use crate::raster::{Grid, min_conductor_path_width, rasterize};
use crate::setup::{SimSettings, Setup, SolverKind};
use crate::solver::{UsedSolver, solve_direct, solve_pcg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Rasterize,
    Build,
    Solve,
    Postprocess,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: Stage,
    /// 0..1 within the stage if known.
    pub fraction: Option<f64>,
    pub message: String,
}

pub type ProgressFn<'a> = &'a (dyn Fn(Progress) + Sync);

/// Per-pixel result fields in image order (row 0 = top), normalized to
/// V(plus) = 1, V(minus) = 0 and a sheet resistance of 1 Ω/□.
pub struct Fields {
    /// Potential (NaN outside the conducting area).
    pub potential: Vec<f32>,
    /// Sheet current through the pixel in +x (right) and +y (down) direction,
    /// in units of "current per pixel width".
    pub jx: Vec<f32>,
    pub jy: Vec<f32>,
    /// Pixel classification, see [`crate::network::material`].
    pub material: Vec<u8>,
    /// Material coverage (conductor or terminal) used for display.
    pub coverage: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub conductor_pixels: usize,
    pub removed_pixels: usize,
    pub unknowns: usize,
    pub nonzeros: usize,
    pub solver: UsedSolver,
    pub iterations: u32,
    pub relative_residual: f64,
    /// Relative mismatch between current leaving plus and entering minus.
    pub current_balance_error: f64,
    pub time_raster_ms: f64,
    pub time_build_ms: f64,
    pub time_solve_ms: f64,
    pub time_post_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub grid: Grid,
    /// Resistance in units of the sheet resistance (number of squares).
    pub squares: f64,
    /// Potential of every terminal (plus = 1, minus = 0, floating = solved).
    pub terminal_potentials: Vec<f64>,
    /// Smallest conductor path width in µm, if the layout contains paths.
    pub min_path_width_um: Option<f64>,
    /// Suggested resolution so the narrowest path spans at least 10 pixels.
    pub suggested_resolution: Option<u32>,
    pub stats: Stats,
    pub warnings: Vec<String>,
}

pub struct SimOutput {
    pub summary: Summary,
    pub fields: Fields,
}

/// Unknown count above which `SolverKind::Auto` switches to the iterative solver.
const AUTO_DIRECT_LIMIT: usize = 8_000_000;
const MIN_PIXELS_PER_WIDTH: f64 = 10.0;

pub fn simulate(
    layout: &FlatLayout,
    setup: &Setup,
    settings: &SimSettings,
    cancel: &AtomicBool,
    progress: ProgressFn,
) -> Result<SimOutput> {
    let problems = setup.validate(layout);
    if !problems.is_empty() {
        return Err(CrazeError::Setup(problems.join("\n")));
    }
    let check = || if cancel.load(std::sync::atomic::Ordering::Relaxed) { Err(CrazeError::Cancelled) } else { Ok(()) };
    let msg = |stage, m: &str| progress(Progress { stage, fraction: None, message: m.into() });

    let t0 = Instant::now();
    msg(Stage::Rasterize, "Rasterizing layout …");
    let raster = rasterize(layout, setup, settings.resolution)?;
    let grid = raster.grid;
    let time_raster_ms = t0.elapsed().as_secs_f64() * 1e3;
    check()?;

    let t1 = Instant::now();
    msg(Stage::Build, "Building resistor network …");
    let names: Vec<String> = setup.terminals.iter().map(|t| t.name.clone()).collect();
    let net = Network::build(&raster, setup.source, setup.sink, &names)?;
    let (a, b) = net.assemble();
    let time_build_ms = t1.elapsed().as_secs_f64() * 1e3;
    check()?;

    let t2 = Instant::now();
    let n = a.n;
    let use_direct = match settings.solver {
        SolverKind::Direct => true,
        SolverKind::Iterative => false,
        SolverKind::Auto => n <= AUTO_DIRECT_LIMIT,
    };
    let (x, used, iterations) = if use_direct {
        msg(Stage::Solve, &format!("Solving {n} unknowns directly …"));
        (solve_direct(&a, &b)?, UsedSolver::Direct, 0)
    } else {
        msg(Stage::Solve, &format!("Solving {n} unknowns iteratively …"));
        let tol = settings.tolerance;
        let cb = |it: u32, rel: f64| {
            let frac = if rel > 0.0 { (rel.log10() / tol.log10()).clamp(0.0, 1.0) } else { 1.0 };
            progress(Progress {
                stage: Stage::Solve,
                fraction: Some(frac),
                message: format!("Iteration {it}, residual {rel:.1e}"),
            });
        };
        let (x, it, _) = solve_pcg(&a, &b, None, tol, settings.max_iterations, cancel, &cb)?;
        (x, UsedSolver::Iterative, it)
    };
    let relative_residual = a.residual_norm(&x, &b);
    let time_solve_ms = t2.elapsed().as_secs_f64() * 1e3;
    check()?;

    let t3 = Instant::now();
    msg(Stage::Postprocess, "Computing results …");
    let term_v = net.terminal_potentials(&x);
    let (fields, i_source, i_sink) = compute_fields(&net, &raster.conductor, &x, &term_v);
    if !(i_source > 0.0) {
        return Err(CrazeError::Solver("No current flows. Is the trace interrupted?".into()));
    }
    let squares = 1.0 / i_source;
    let current_balance_error = ((i_source - i_sink) / i_source).abs();
    let time_post_ms = t3.elapsed().as_secs_f64() * 1e3;

    let mut warnings = net.warnings.clone();
    let min_w = min_conductor_path_width(layout, setup);
    let mut suggested = None;
    if let Some(w) = min_w {
        let px_per_w = w / grid.px_um;
        let needed = (settings.resolution as f64 * MIN_PIXELS_PER_WIDTH / px_per_w).ceil() as u32;
        if px_per_w < MIN_PIXELS_PER_WIDTH {
            suggested = Some(needed.div_ceil(100) * 100);
            warnings.push(format!(
                "The narrowest trace is only {px_per_w:.1} pixels wide. Use a resolution of at least {} pixels for accurate results.",
                needed.div_ceil(100) * 100
            ));
        }
    }

    Ok(SimOutput {
        summary: Summary {
            grid,
            squares,
            terminal_potentials: term_v,
            min_path_width_um: min_w,
            suggested_resolution: suggested,
            stats: Stats {
                conductor_pixels: net.total_conductor_nodes,
                removed_pixels: net.removed_nodes,
                unknowns: n,
                nonzeros: a.nnz(),
                solver: used,
                iterations,
                relative_residual,
                current_balance_error,
                time_raster_ms,
                time_build_ms,
                time_solve_ms,
                time_post_ms,
            },
            warnings,
        },
        fields,
    })
}

/// Returns fields, current leaving the plus terminal and current entering the minus terminal.
fn compute_fields(net: &Network, conductor: &[f32], x: &[f64], term_v: &[f64]) -> (Fields, f64, f64) {
    let n_pix = net.nx * net.ny;
    let mut potential = vec![f32::NAN; n_pix];
    let mut jx = vec![0f32; n_pix];
    let mut jy = vec![0f32; n_pix];
    let mut coverage = vec![0f32; n_pix];

    for i in 0..n_pix {
        let t = net.term_of_pixel[i];
        if t >= 0 {
            potential[i] = term_v[t as usize] as f32;
            coverage[i] = net.term_cov[i];
        } else {
            coverage[i] = conductor[i];
        }
    }

    // Flow out of node j towards neighbour k.
    let flows: Vec<[f64; 4]> = (0..net.node_pixel.len())
        .into_par_iter()
        .map(|j| {
            let vj = x[j];
            let mut f = [0.0; 4];
            for k in 0..4 {
                let vn = match net.nb[j][k] {
                    Neighbor::Node(m) => x[m as usize],
                    Neighbor::Terminal(t) => term_v[t as usize],
                    Neighbor::None => continue,
                };
                f[k] = net.g[j][k] * (vj - vn);
            }
            f
        })
        .collect();

    let mut i_source = 0.0;
    let mut i_sink = 0.0;
    for (j, f) in flows.iter().enumerate() {
        let p = net.node_pixel[j] as usize;
        potential[p] = x[j] as f32;
        // Directions: 0 up, 1 down, 2 left, 3 right (image coordinates).
        jx[p] = (0.5 * (f[3] - f[2])) as f32;
        jy[p] = (0.5 * (f[1] - f[0])) as f32;
        for k in 0..4 {
            if let Neighbor::Terminal(t) = net.nb[j][k] {
                if t as usize == net.source {
                    i_source -= f[k];
                } else if t as usize == net.sink {
                    i_sink += f[k];
                }
            }
        }
    }
    debug_assert_eq!(DIRS[3], (1, 0));
    (Fields { potential, jx, jy, material: net.material.clone(), coverage }, i_source, i_sink)
}
