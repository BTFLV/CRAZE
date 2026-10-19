//! Exact-area rasterization of layout layers into per-pixel coverage maps.
//!
//! Overlapping shapes of one role are merged (boolean union) first, so every pixel gets the
//! exact fraction of its area covered by material. The coverage itself is computed with a
//! signed-area accumulation scan converter, which is exact for straight polygon edges.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use rayon::prelude::*;
use serde::Serialize;

use crate::error::{CrazeError, Result};
use crate::gds::{FlatLayout, LayerKey};
use crate::geometry::{BBox, Pt};
use crate::setup::{Role, Setup};

/// Pixel grid placement in layout coordinates.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    /// Edge length of one (square) pixel in µm.
    pub px_um: f64,
    /// Layout x of the left image border (µm).
    pub x0_um: f64,
    /// Layout y of the top image border (µm). Image rows grow downwards.
    pub y0_um: f64,
}

impl Grid {
    pub fn to_px(&self, p: Pt) -> [f64; 2] {
        [(p.x - self.x0_um) / self.px_um, (self.y0_um - p.y) / self.px_um]
    }
    pub fn len(&self) -> usize {
        self.nx * self.ny
    }
}

#[derive(Debug, Clone)]
pub struct Raster {
    pub grid: Grid,
    /// Conductor coverage per pixel (0..1).
    pub conductor: Vec<f32>,
    /// Coverage per pixel for every terminal definition.
    pub terminals: Vec<Vec<f32>>,
}

/// Number of empty pixels added around an automatically determined simulation area.
const AUTO_MARGIN_PX: usize = 2;

/// Determines the simulated area. Uses the region layers if present, otherwise the
/// bounding box of conductor and terminal geometry. Returns (bbox, is_explicit_region).
pub fn simulation_area(layout: &FlatLayout, setup: &Setup) -> Option<(BBox, bool)> {
    let mut region = BBox::empty();
    let mut auto = BBox::empty();
    for (key, shapes) in &layout.shapes {
        let role = setup.role_of(*key);
        for s in shapes {
            let b = BBox::of(&s.polygon);
            match role {
                Role::Region => region.union(&b),
                Role::Conductor | Role::Terminal { .. } => auto.union(&b),
                Role::Ignore => {}
            }
        }
    }
    if region.is_valid() {
        Some((region, true))
    } else if auto.is_valid() {
        Some((auto, false))
    } else {
        None
    }
}

pub fn make_grid(area: BBox, explicit: bool, resolution: u32) -> Grid {
    let res = resolution.max(8) as f64;
    let px = area.width().max(area.height()) / res;
    let margin = if explicit { 0 } else { AUTO_MARGIN_PX };
    let nx = (area.width() / px).ceil().max(1.0) as usize + 2 * margin;
    let ny = (area.height() / px).ceil().max(1.0) as usize + 2 * margin;
    Grid { nx, ny, px_um: px, x0_um: area.min.x - margin as f64 * px, y0_um: area.max.y + margin as f64 * px }
}

pub fn rasterize(layout: &FlatLayout, setup: &Setup, resolution: u32) -> Result<Raster> {
    let (area, explicit) = simulation_area(layout, setup)
        .ok_or_else(|| CrazeError::Setup("No geometry found on conductor or terminal layers.".into()))?;
    let grid = make_grid(area, explicit, resolution);
    if grid.len() > 60_000_000 {
        return Err(CrazeError::Setup("The resolution is too high (more than 60 million pixels).".into()));
    }
    let keys_for = |pred: &dyn Fn(Role) -> bool| -> Vec<LayerKey> { setup.keys_with(pred) };
    let conductor = rasterize_layers(layout, &keys_for(&|r| r == Role::Conductor), &grid);
    let terminals = (0..setup.terminals.len())
        .map(|id| rasterize_layers(layout, &keys_for(&|r| r == Role::Terminal { id }), &grid))
        .collect();
    Ok(Raster { grid, conductor, terminals })
}

/// Merges all shapes of `keys`, clips them to the grid and returns per-pixel coverage.
pub fn rasterize_layers(layout: &FlatLayout, keys: &[LayerKey], grid: &Grid) -> Vec<f32> {
    let contours: Vec<Vec<[f64; 2]>> = keys
        .iter()
        .filter_map(|k| layout.shapes.get(k))
        .flatten()
        .map(|s| {
            // GDSII does not prescribe an orientation. Give every contour the same one so the
            // NonZero fill rule merges overlapping shapes instead of cancelling them.
            let mut c: Vec<[f64; 2]> = s.polygon.iter().map(|p| grid.to_px(*p)).collect();
            if signed_area_px(&c) < 0.0 {
                c.reverse();
            }
            c
        })
        .collect();
    if contours.is_empty() {
        return vec![0.0; grid.len()];
    }
    let (w, h) = (grid.nx as f64, grid.ny as f64);
    let rect = vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let merged = contours.overlay(&rect, OverlayRule::Intersect, FillRule::NonZero);
    let edges: Vec<[[f64; 2]; 2]> = merged
        .iter()
        .flatten()
        .flat_map(|c| (0..c.len()).map(move |i| [c[i], c[(i + 1) % c.len()]]))
        .collect();
    coverage_from_edges(&edges, grid.nx, grid.ny)
}

fn signed_area_px(c: &[[f64; 2]]) -> f64 {
    let n = c.len();
    (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum::<f64>() * 0.5
}

/// Scan converts closed contours (given as edges) into exact area coverage per pixel.
pub fn coverage_from_edges(edges: &[[[f64; 2]; 2]], nx: usize, ny: usize) -> Vec<f32> {
    let stride = nx + 2;
    let band = 32usize;
    let mut cov = vec![0f32; nx * ny];
    cov.par_chunks_mut(nx * band).enumerate().for_each(|(bi, out)| {
        let y_start = bi * band;
        let rows = out.len() / nx;
        let mut acc = vec![0f64; stride * rows];
        for e in edges {
            draw_line(&mut acc, stride, y_start, y_start + rows, nx as f64, e[0], e[1]);
        }
        for r in 0..rows {
            let mut s = 0.0;
            for x in 0..nx {
                s += acc[r * stride + x];
                out[r * nx + x] = (s.abs().min(1.0)) as f32;
            }
        }
    });
    cov
}

/// Accumulates the signed area contribution of one edge into rows [y_begin, y_end).
fn draw_line(acc: &mut [f64], stride: usize, y_begin: usize, y_end: usize, w: f64, a: [f64; 2], b: [f64; 2]) {
    let cl = |v: f64| v.clamp(0.0, w);
    let (a, b) = ([cl(a[0]), a[1]], [cl(b[0]), b[1]]);
    if (a[1] - b[1]).abs() < 1e-12 {
        return;
    }
    let (dir, p0, p1) = if a[1] < b[1] { (1.0, a, b) } else { (-1.0, b, a) };
    let dxdy = (p1[0] - p0[0]) / (p1[1] - p0[1]);
    let row_first = (p0[1].floor().max(0.0) as usize).max(y_begin);
    let row_last = (p1[1].ceil().max(0.0) as usize).min(y_end);
    if row_first >= row_last {
        return;
    }
    let ys = (row_first as f64).max(p0[1]);
    let mut x = p0[0] + (ys - p0[1]) * dxdy;
    for y in row_first..row_last {
        let yt = (y as f64).max(p0[1]);
        let yb = ((y + 1) as f64).min(p1[1]);
        let dy = yb - yt;
        if dy <= 0.0 {
            continue;
        }
        let xnext = cl(x + dxdy * dy);
        let d = dy * dir;
        let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
        let x0f = x0.floor();
        let x0i = x0f as usize;
        let x1c = x1.ceil();
        let x1i = x1c as usize;
        let row = (y - y_begin) * stride;
        if x1i <= x0i + 1 {
            let xmf = 0.5 * (x + xnext) - x0f;
            acc[row + x0i] += d - d * xmf;
            acc[row + x0i + 1] += d * xmf;
        } else {
            let s = 1.0 / (x1 - x0);
            let x0r = x0 - x0f;
            let a0 = 0.5 * s * (1.0 - x0r) * (1.0 - x0r);
            let x1r = x1 - x1c + 1.0;
            let am = 0.5 * s * x1r * x1r;
            acc[row + x0i] += d * a0;
            if x1i == x0i + 2 {
                acc[row + x0i + 1] += d * (1.0 - a0 - am);
            } else {
                let a1 = s * (1.5 - x0r);
                acc[row + x0i + 1] += d * (a1 - a0);
                for xi in x0i + 2..x1i - 1 {
                    acc[row + xi] += d * s;
                }
                let a2 = a1 + (x1i - x0i - 3) as f64 * s;
                acc[row + x1i - 1] += d * (1.0 - a2 - am);
            }
            acc[row + x1i] += d * am;
        }
        x = xnext;
    }
}

/// Smallest path width on the conductor layers in µm (None if there are no paths).
pub fn min_conductor_path_width(layout: &FlatLayout, setup: &Setup) -> Option<f64> {
    setup
        .conductor_keys()
        .iter()
        .filter_map(|k| layout.shapes.get(k))
        .flatten()
        .filter(|s| s.path_width > 0.0)
        .map(|s| s.path_width)
        .min_by(|a, b| a.total_cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(edges: &[[[f64; 2]; 2]], nx: usize, ny: usize) -> f64 {
        coverage_from_edges(edges, nx, ny).iter().map(|v| *v as f64).sum()
    }

    fn poly_edges(p: &[[f64; 2]]) -> Vec<[[f64; 2]; 2]> {
        (0..p.len()).map(|i| [p[i], p[(i + 1) % p.len()]]).collect()
    }

    #[test]
    fn triangle_area_exact() {
        let tri = [[1.3, 2.7], [37.9, 5.1], [12.2, 61.4]];
        let exact: f64 = 0.5 * ((37.9f64 - 1.3) * (61.4 - 2.7) - (12.2 - 1.3) * (5.1 - 2.7)).abs();
        let a = total(&poly_edges(&tri), 64, 70);
        assert!((a - exact).abs() < 1e-3, "{a} vs {exact}");
    }

    #[test]
    fn partial_pixel_values() {
        // Square from (0.25,0.5) to (2.75,1.5): row 0 lower half and row 1 upper half covered.
        let sq = [[0.25, 0.5], [2.75, 0.5], [2.75, 1.5], [0.25, 1.5]];
        let c = coverage_from_edges(&poly_edges(&sq), 3, 2);
        let expect = [0.375, 0.5, 0.375, 0.375, 0.5, 0.375];
        for (v, e) in c.iter().zip(expect) {
            assert!((*v as f64 - e).abs() < 1e-6, "{c:?}");
        }
    }

    #[test]
    fn overlap_with_opposite_orientation_is_merged() {
        use crate::gds::{FlatShape, ShapeKind};
        use std::collections::BTreeMap;
        let key = LayerKey { layer: 2, datatype: 0 };
        let rect = |x0: f64, x1: f64| vec![Pt::new(x0, 0.0), Pt::new(x1, 0.0), Pt::new(x1, 4.0), Pt::new(x0, 4.0)];
        let mut cw = rect(4.0, 10.0);
        cw.reverse();
        let shape = |polygon| FlatShape { kind: ShapeKind::Polygon, polygon, path_width: 0.0 };
        let layout = FlatLayout {
            top_cell: "T".into(),
            shapes: BTreeMap::from([(key, vec![shape(rect(0.0, 6.0)), shape(cw)])]),
            labels: vec![],
        };
        let grid = Grid { nx: 12, ny: 6, px_um: 1.0, x0_um: -1.0, y0_um: 5.0 };
        let a: f64 = rasterize_layers(&layout, &[key], &grid).iter().map(|v| *v as f64).sum();
        assert!((a - 40.0).abs() < 1e-6, "{a}");
    }

    #[test]
    fn hole_is_subtracted() {
        // Outer CCW, hole CW.
        let outer = [[1.0, 1.0], [9.0, 1.0], [9.0, 9.0], [1.0, 9.0]];
        let hole = [[3.0, 3.0], [3.0, 6.5], [6.5, 6.5], [6.5, 3.0]];
        let mut e = poly_edges(&outer);
        e.extend(poly_edges(&hole));
        let a = total(&e, 10, 10);
        assert!((a - (64.0 - 12.25)).abs() < 1e-6, "{a}");
    }
}
