//! Command line interface for CRAZE (batch simulations, scripting, benchmarks).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand};
use craze_core::gds::{LayerKey, Library, records};
use craze_core::geometry::signed_area;
use craze_core::materials::Stack;
use craze_core::optimize::{OptimizeParams, optimize};
use craze_core::setup::{LayerAssignment, Role, SimSettings, Setup, SolverKind, TerminalDef};
use craze_core::simulate::simulate;

#[derive(Parser)]
#[command(name = "craze", version, about = "CRAZE – 2D resistance simulation for thin-film layouts (GDSII)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args)]
struct LayoutArgs {
    /// GDSII file (binary or KLayout GDS2 text)
    file: PathBuf,
    /// Top cell (default: the first top cell)
    #[arg(long)]
    top: Option<String>,
    /// Conductor layer, e.g. 2/0 (repeatable). Without --conductor and --terminal the automatic suggestion is used.
    #[arg(long = "conductor", value_parser = parse_key)]
    conductors: Vec<LayerKey>,
    /// Terminal layers (repeatable): the first terminal is plus, the second minus, further ones float. Several layers per terminal separated by commas.
    #[arg(long = "terminal")]
    terminals: Vec<String>,
    /// Layer that defines the simulation area (optional)
    #[arg(long, value_parser = parse_key)]
    region: Option<LayerKey>,
    /// Resolution (pixels along the longer side)
    #[arg(long, default_value_t = 1000)]
    res: u32,
    /// Solver: auto, direct, iterative
    #[arg(long, default_value = "auto")]
    solver: String,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show cells and layers of a GDSII file
    Info { file: PathBuf },
    /// Simulate the resistance
    Simulate {
        #[command(flatten)]
        layout: LayoutArgs,
        /// Sheet resistance in Ω/□ (default: 100 nm Pt on 10 nm Ta at 20 °C)
        #[arg(long)]
        rsheet: Option<f64>,
        /// Print the result as JSON
        #[arg(long)]
        json: bool,
    },
    /// Simulate at several resolutions (convergence study)
    Sweep {
        #[command(flatten)]
        layout: LayoutArgs,
        /// Comma-separated resolutions
        #[arg(long, default_value = "250,500,1000,2000")]
        resolutions: String,
    },
    /// Scale path widths to reach a target resistance
    Optimize {
        #[command(flatten)]
        layout: LayoutArgs,
        #[arg(long)]
        target: f64,
        #[arg(long)]
        rsheet: Option<f64>,
        /// Relative tolerance
        #[arg(long, default_value_t = 0.002)]
        tol: f64,
        #[arg(long, default_value_t = 10)]
        iterations: u32,
        /// Output file (binary GDSII)
        #[arg(long)]
        out: PathBuf,
    },
}

fn parse_key(s: &str) -> Result<LayerKey, String> {
    let (l, d) = s.split_once('/').unwrap_or((s, "0"));
    Ok(LayerKey {
        layer: l.trim().parse().map_err(|_| format!("invalid layer '{s}'"))?,
        datatype: d.trim().parse().map_err(|_| format!("invalid datatype '{s}'"))?,
    })
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn load(file: &PathBuf) -> Result<Library, Box<dyn std::error::Error>> {
    Ok(Library::from_bytes(&std::fs::read(file)?)?)
}

fn prepare(a: &LayoutArgs) -> Result<(Library, String, Setup, SimSettings), Box<dyn std::error::Error>> {
    let lib = load(&a.file)?;
    let top = a.top.clone().or_else(|| lib.top_cells().first().cloned()).ok_or("no top cell")?;
    let flat = lib.flatten(&top)?;
    let setup = if a.conductors.is_empty() && a.terminals.is_empty() {
        Setup::suggest(&flat)
    } else {
        let mut layers: Vec<LayerAssignment> =
            a.conductors.iter().map(|&key| LayerAssignment { key, role: Role::Conductor }).collect();
        for (id, t) in a.terminals.iter().enumerate() {
            for k in t.split(',') {
                layers.push(LayerAssignment { key: parse_key(k)?, role: Role::Terminal { id } });
            }
        }
        if let Some(key) = a.region {
            layers.push(LayerAssignment { key, role: Role::Region });
        }
        let terminals = (0..a.terminals.len().max(2)).map(|i| TerminalDef { name: format!("T{}", i + 1) }).collect();
        Setup { layers, terminals, source: 0, sink: 1 }
    };
    let solver = match a.solver.as_str() {
        "direct" => SolverKind::Direct,
        "iterative" => SolverKind::Iterative,
        _ => SolverKind::Auto,
    };
    let settings = SimSettings { resolution: a.res, solver, ..Default::default() };
    Ok((lib, top, setup, settings))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cancel = AtomicBool::new(false);
    match cli.cmd {
        Cmd::Info { file } => {
            let lib = load(&file)?;
            println!("Library: {}  (1 database unit = {:e} m)", lib.name, lib.db_unit_m);
            println!("Top cells: {}", lib.top_cells().join(", "));
            for top in lib.top_cells() {
                let flat = lib.flatten(&top)?;
                println!("\nCell {top}:");
                for (k, shapes) in &flat.shapes {
                    let area: f64 = shapes.iter().map(|s| signed_area(&s.polygon).abs()).sum();
                    let paths = shapes.iter().filter(|s| s.path_width > 0.0).count();
                    println!("  Layer {k:>7}: {:5} shapes ({paths} paths), area {area:.1} µm²", shapes.len());
                }
                let s = Setup::suggest(&flat);
                println!("  Suggestion: {}", serde_json::to_string(&s.layers)?);
            }
        }
        Cmd::Simulate { layout, rsheet, json } => {
            let (lib, top, setup, settings) = prepare(&layout)?;
            let flat = lib.flatten(&top)?;
            let out = simulate(&flat, &setup, &settings, &cancel, &|p| eprintln!("  {}", p.message))?;
            let rs = rsheet.unwrap_or_else(|| Stack::default().sheet_resistance(20.0));
            if json {
                println!("{}", serde_json::to_string_pretty(&out.summary)?);
            } else {
                let s = &out.summary;
                println!("Squares:       {:.6}", s.squares);
                println!("Resistance:    {:.4} Ω  (R□ = {rs:.4} Ω/□)", s.squares * rs);
                println!("Grid:          {} × {} pixels of {:.4} µm", s.grid.nx, s.grid.ny, s.grid.px_um);
                println!(
                    "Unknowns:      {} ({:?}, residual {:.1e}, current balance {:.1e})",
                    s.stats.unknowns, s.stats.solver, s.stats.relative_residual, s.stats.current_balance_error
                );
                println!(
                    "Time:          raster {:.0} ms, network {:.0} ms, solve {:.0} ms, post {:.0} ms",
                    s.stats.time_raster_ms, s.stats.time_build_ms, s.stats.time_solve_ms, s.stats.time_post_ms
                );
                for w in &s.warnings {
                    println!("Note:          {w}");
                }
            }
        }
        Cmd::Sweep { layout, resolutions } => {
            let (lib, top, setup, mut settings) = prepare(&layout)?;
            let flat = lib.flatten(&top)?;
            println!("{:>8} {:>14} {:>10} {:>10}", "Pixels", "Squares", "Unknowns", "Time/ms");
            for r in resolutions.split(',') {
                settings.resolution = r.trim().parse()?;
                let out = simulate(&flat, &setup, &settings, &cancel, &|_| {})?;
                let st = &out.summary.stats;
                let t = st.time_raster_ms + st.time_build_ms + st.time_solve_ms + st.time_post_ms;
                println!("{:>8} {:>14.6} {:>10} {:>10.0}", settings.resolution, out.summary.squares, st.unknowns, t);
            }
        }
        Cmd::Optimize { layout, target, rsheet, tol, iterations, out } => {
            let (lib, top, setup, settings) = prepare(&layout)?;
            let rs = rsheet.unwrap_or_else(|| Stack::default().sheet_resistance(20.0));
            let params = OptimizeParams {
                library: &lib,
                top_cell: &top,
                setup: &setup,
                settings: &settings,
                sheet_resistance: rs,
                target_ohm: target,
                tolerance: tol,
                max_iterations: iterations,
            };
            let res = optimize(&params, &cancel, &|_| {}, &|s| {
                println!("Step {}: width factor {:.5} → {:.3} Ω", s.iteration, s.scale, s.resistance)
            })?;
            let keys: HashSet<LayerKey> = setup.conductor_keys().into_iter().collect();
            std::fs::write(&out, records::write_binary(&lib.with_scaled_path_widths(&keys, res.best_scale)))?;
            println!(
                "{} factor {:.5}, R = {:.3} Ω → {}",
                if res.converged { "Converged:" } else { "Not converged, best result:" },
                res.best_scale,
                res.best_resistance,
                out.display()
            );
        }
    }
    Ok(())
}
