//! Validation against KLayout (geometry) and analytic solutions (physics).

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::sync::atomic::AtomicBool;

use craze_core::gds::{FlatLayout, FlatShape, LayerKey, Library, ShapeKind, records};
use craze_core::geometry::{Pt, signed_area};
use craze_core::raster::{Grid, rasterize_layers};
use craze_core::setup::{LayerAssignment, Role, SimSettings, Setup, SolverKind, TerminalDef};
use craze_core::simulate::simulate;

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

fn key(l: i16, d: i16) -> LayerKey {
    LayerKey { layer: l, datatype: d }
}

fn merged_area(flat: &FlatLayout, k: LayerKey) -> f64 {
    // Rasterize on a fine grid; union area = sum of coverage * px².
    let mut bb = craze_core::geometry::BBox::empty();
    for s in &flat.shapes[&k] {
        bb.union(&craze_core::geometry::BBox::of(&s.polygon));
    }
    let px = bb.width().max(bb.height()) / 4000.0;
    let grid = Grid {
        nx: (bb.width() / px).ceil() as usize + 2,
        ny: (bb.height() / px).ceil() as usize + 2,
        px_um: px,
        x0_um: bb.min.x - px,
        y0_um: bb.max.y + px,
    };
    rasterize_layers(flat, &[k], &grid).iter().map(|v| *v as f64).sum::<f64>() * px * px
}

#[test]
fn hierarchy_matches_klayout() {
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{DATA}/hier_ref.json")).unwrap()).unwrap();
    for file in ["hier.gds", "hier.txt"] {
        let lib = Library::from_bytes(&std::fs::read(format!("{DATA}/{file}")).unwrap()).unwrap();
        assert_eq!(lib.top_cells(), vec!["TOP".to_string()]);
        let flat = lib.flatten("TOP").unwrap();
        for (k, expect) in reference["areas_um2"].as_object().unwrap() {
            let (l, d) = k.split_once('/').unwrap();
            let a = merged_area(&flat, key(l.parse().unwrap(), d.parse().unwrap()));
            let e = expect.as_f64().unwrap();
            assert!((a - e).abs() / e < 2e-3, "{file} layer {k}: {a} vs KLayout {e}");
        }
    }
}

#[test]
fn binary_roundtrip() {
    let bytes = std::fs::read(format!("{DATA}/hier.gds")).unwrap();
    let recs = records::read_binary(&bytes).unwrap();
    let again = records::read_binary(&records::write_binary(&recs)).unwrap();
    assert_eq!(recs, again);
    for v in [1e-9, 1e-3, 0.001, 2.0, 90.0, 123.456e-7] {
        assert!((records::decode_real8(records::encode_real8(v)) - v).abs() <= v.abs() * 1e-15);
    }
}

#[test]
fn text_and_binary_examples_agree() {
    for name in ["2_paths", "8_paths", "G1_R4", "crowd_4_1"] {
        let a = Library::from_bytes(&std::fs::read(format!("{DATA}/{name}.gds")).unwrap()).unwrap();
        let b = Library::from_bytes(&std::fs::read(format!("{DATA}/../../../../examples/{name}.gds2")).unwrap()).unwrap();
        let fa = a.flatten(&a.top_cells()[0]).unwrap();
        let fb = b.flatten(&b.top_cells()[0]).unwrap();
        assert_eq!(fa.shapes.keys().collect::<Vec<_>>(), fb.shapes.keys().collect::<Vec<_>>());
        for k in fa.shapes.keys() {
            let sa: f64 = fa.shapes[k].iter().map(|s| signed_area(&s.polygon).abs()).sum();
            let sb: f64 = fb.shapes[k].iter().map(|s| signed_area(&s.polygon).abs()).sum();
            assert!((sa - sb).abs() <= 1e-9 * sa.max(1.0), "{name} {k}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------------------

struct Builder {
    shapes: BTreeMap<LayerKey, Vec<FlatShape>>,
}

impl Builder {
    fn new() -> Self {
        Self { shapes: BTreeMap::new() }
    }
    fn poly(mut self, layer: i16, pts: Vec<Pt>) -> Self {
        self.shapes.entry(key(layer, 0)).or_default().push(FlatShape { kind: ShapeKind::Polygon, polygon: pts, path_width: 0.0 });
        self
    }
    fn build(self) -> FlatLayout {
        FlatLayout { top_cell: "T".into(), shapes: self.shapes, labels: vec![] }
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Pt> {
    vec![Pt::new(x0, y0), Pt::new(x1, y0), Pt::new(x1, y1), Pt::new(x0, y1)]
}

fn rotated(pts: Vec<Pt>, deg: f64) -> Vec<Pt> {
    let (s, c) = deg.to_radians().sin_cos();
    pts.into_iter().map(|p| Pt::new(c * p.x - s * p.y, s * p.x + c * p.y)).collect()
}

fn circle(r: f64, n: usize) -> Vec<Pt> {
    (0..n).map(|i| {
        let a = 2.0 * PI * i as f64 / n as f64;
        Pt::new(r * a.cos(), r * a.sin())
    }).collect()
}

fn standard_setup() -> Setup {
    Setup {
        layers: vec![
            LayerAssignment { key: key(2, 0), role: Role::Conductor },
            LayerAssignment { key: key(3, 0), role: Role::Terminal { id: 0 } },
            LayerAssignment { key: key(4, 0), role: Role::Terminal { id: 1 } },
        ],
        terminals: vec![TerminalDef { name: "Plus".into() }, TerminalDef { name: "Minus".into() }],
        source: 0,
        sink: 1,
    }
}

fn squares(flat: &FlatLayout, setup: &Setup, res: u32, solver: SolverKind) -> f64 {
    let settings = SimSettings { resolution: res, solver, ..Default::default() };
    let out = simulate(flat, setup, &settings, &AtomicBool::new(false), &|_| {}).unwrap();
    assert!(out.summary.stats.current_balance_error < 1e-6, "{:?}", out.summary.stats);
    out.summary.squares
}

#[test]
fn straight_bar_exact() {
    // 100 x 10 bar, contacts are 2 µm wide pads covering the bar ends.
    let flat = Builder::new()
        .poly(2, rect(0.0, 0.0, 100.0, 10.0))
        .poly(3, rect(-2.0, 0.0, 0.0, 10.0))
        .poly(4, rect(100.0, 0.0, 102.0, 10.0))
        .build();
    let n = squares(&flat, &standard_setup(), 520, SolverKind::Direct);
    assert!((n - 10.0).abs() / 10.0 < 2e-3, "{n}");
}

#[test]
fn rotated_bar() {
    // Same bar rotated by 30°: the sub-pixel model must keep the error small.
    let flat = Builder::new()
        .poly(2, rotated(rect(0.0, 0.0, 100.0, 10.0), 30.0))
        .poly(3, rotated(rect(-2.0, -1.0, 0.0, 11.0), 30.0))
        .poly(4, rotated(rect(100.0, -1.0, 102.0, 11.0), 30.0))
        .build();
    for res in [400, 800] {
        let n = squares(&flat, &standard_setup(), res, SolverKind::Direct);
        eprintln!("rotated bar res {res}: {n:.5} squares (exact 10)");
        assert!((n - 10.0).abs() / 10.0 < 0.02, "res {res}: {n}");
    }
}

#[test]
fn corbino_ring() {
    // Annulus r1 = 10, r2 = 40: R = ln(r2/r1) / (2π) squares.
    let (r1, r2) = (10.0, 40.0);
    let mut hole = circle(r1, 720);
    hole.reverse();
    let mut outer_hole = circle(r2, 720);
    outer_hole.reverse();
    let flat = Builder::new()
        .poly(2, circle(r2 + 0.5, 720))
        .poly(3, circle(r1, 720))
        .build();
    // Outer contact: ring between r2 and r2+5 (as two half shapes would also work; use a polygon with hole via union of slices).
    let mut b = Builder { shapes: flat.shapes };
    for i in 0..36 {
        let a0 = 2.0 * PI * i as f64 / 36.0;
        let a1 = 2.0 * PI * (i + 1) as f64 / 36.0 + 0.01;
        let mut seg = Vec::new();
        for k in 0..=10 {
            let a = a0 + (a1 - a0) * k as f64 / 10.0;
            seg.push(Pt::new(r2 * a.cos(), r2 * a.sin()));
        }
        for k in (0..=10).rev() {
            let a = a0 + (a1 - a0) * k as f64 / 10.0;
            seg.push(Pt::new((r2 + 5.0) * a.cos(), (r2 + 5.0) * a.sin()));
        }
        b = b.poly(4, seg);
    }
    let flat = b.build();
    let exact = (r2 / r1).ln() / (2.0 * PI);
    for res in [300, 600] {
        let n = squares(&flat, &standard_setup(), res, SolverKind::Direct);
        eprintln!("corbino res {res}: {n:.5} squares (exact {exact:.5})");
        assert!((n - exact).abs() / exact < 0.02, "res {res}: {n} vs {exact}");
    }
}

#[test]
fn right_angle_corner() {
    // L-shaped trace, width 10, centerline legs 60 each. Known result:
    // N = (straight length)/W + 0.559 for the corner square.
    let w = 10.0;
    let flat = Builder::new()
        .poly(2, vec![Pt::new(0.0, 0.0), Pt::new(70.0, 0.0), Pt::new(70.0, 70.0), Pt::new(60.0, 70.0), Pt::new(60.0, 10.0), Pt::new(0.0, 10.0)])
        .poly(3, rect(-2.0, 0.0, 0.0, 10.0))
        .poly(4, rect(60.0, 70.0, 70.0, 72.0))
        .build();
    let expected = 60.0 / w + 60.0 / w + 0.559;
    let n = squares(&flat, &standard_setup(), 700, SolverKind::Direct);
    eprintln!("corner: {n:.4} squares (reference {expected:.4})");
    assert!((n - expected).abs() / expected < 5e-3, "{n} vs {expected}");
}

#[test]
fn iterative_matches_direct() {
    let flat = Builder::new()
        .poly(2, rotated(rect(0.0, 0.0, 100.0, 10.0), 17.0))
        .poly(3, rotated(rect(-2.0, -1.0, 0.0, 11.0), 17.0))
        .poly(4, rotated(rect(100.0, -1.0, 102.0, 11.0), 17.0))
        .build();
    let d = squares(&flat, &standard_setup(), 500, SolverKind::Direct);
    let i = squares(&flat, &standard_setup(), 500, SolverKind::Iterative);
    assert!((d - i).abs() / d < 1e-7, "{d} vs {i}");
}

#[test]
fn floating_terminal_bridges_gap() {
    // Bar interrupted by a floating ideal contact: R must equal the sum of both halves.
    let mut setup = standard_setup();
    setup.terminals.push(TerminalDef { name: "Mitte".into() });
    setup.layers.push(LayerAssignment { key: key(5, 0), role: Role::Terminal { id: 2 } });
    let flat = Builder::new()
        .poly(2, rect(0.0, 0.0, 45.0, 10.0))
        .poly(2, rect(55.0, 0.0, 100.0, 10.0))
        .poly(5, rect(45.0, 0.0, 55.0, 10.0))
        .poly(3, rect(-2.0, 0.0, 0.0, 10.0))
        .poly(4, rect(100.0, 0.0, 102.0, 10.0))
        .build();
    let n = squares(&flat, &setup, 520, SolverKind::Direct);
    assert!((n - 9.0).abs() / 9.0 < 3e-3, "{n}");
}

#[test]
fn examples_simulate() {
    for name in ["2_paths", "8_paths", "G1_R4", "crowd_4_1"] {
        let lib = Library::from_bytes(&std::fs::read(format!("{DATA}/{name}.gds")).unwrap()).unwrap();
        let flat = lib.flatten(&lib.top_cells()[0]).unwrap();
        let setup = Setup::suggest(&flat);
        assert_eq!(setup.role_of(key(2, 2)), Role::Conductor, "{name}: {:?}", setup.layers);
        let settings = SimSettings { resolution: 1000, ..Default::default() };
        let out = simulate(&flat, &setup, &settings, &AtomicBool::new(false), &|_| {}).unwrap();
        let s = &out.summary;
        eprintln!(
            "{name}: {:.3} squares, {} unknowns, solve {:.0} ms, total {:.0} ms, warnings {:?}",
            s.squares,
            s.stats.unknowns,
            s.stats.time_solve_ms,
            s.stats.time_raster_ms + s.stats.time_build_ms + s.stats.time_solve_ms + s.stats.time_post_ms,
            s.warnings
        );
        assert!(s.squares > 0.0 && s.squares.is_finite());
    }
}
