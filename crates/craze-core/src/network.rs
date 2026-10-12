//! Resistor network on the pixel grid (finite-volume discretization of the thin film).
//!
//! Every pixel with conductor coverage becomes a node. Partially covered pixels are modeled
//! with a straight sub-pixel material edge (angle + distance estimated from the 3×3
//! neighbourhood), which yields the conducting length of each pixel edge. The conductance
//! between two neighbouring nodes is the mean conducting edge length divided by the center
//! distance. Terminal pixels are ideal contacts: all pixels of one terminal share one
//! potential (fixed for plus/minus, unknown for floating terminals).
//!
//! Conductances are in units of the sheet conductance (1/R□), so solving with a potential
//! difference of 1 directly yields the resistance in squares.

use rayon::prelude::*;

use crate::error::{CrazeError, Result};
use crate::raster::Raster;
use crate::solver::Csr;

/// Neighbour directions in image coordinates: up, down, left, right.
pub const DIRS: [(i64, i64); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
const OPPOSITE: [usize; 4] = [1, 0, 3, 2];
const EPS_AREA: f64 = 1e-6;

/// Geometry of the material inside one pixel (local coordinates, pixel = [-0.5, 0.5]²).
#[derive(Debug, Clone, Copy, Default)]
pub struct PixelGeometry {
    pub area: f64,
    /// Direction of the material-edge normal pointing into the material.
    pub angle: f64,
    /// Signed distance of the material edge from the pixel center along the normal.
    pub distance: f64,
    /// Conducting length on the 4 pixel edges (up, down, left, right).
    pub edge_len: [f64; 4],
}

impl PixelGeometry {
    fn full() -> Self {
        Self { area: 1.0, angle: 0.0, distance: -0.5, edge_len: [1.0; 4] }
    }
}

/// Intersections of the line n·p = d with the unit pixel border.
fn line_box_intersections(angle: f64, d: f64) -> Vec<[f64; 2]> {
    let (s, c) = angle.sin_cos();
    let eps = 1e-9;
    let mut out = Vec::with_capacity(4);
    if s.abs() > eps {
        for x in [-0.5, 0.5] {
            let y = (d - x * c) / s;
            if (-0.5 - eps..=0.5 + eps).contains(&y) {
                out.push([x, y]);
            }
        }
    }
    if c.abs() > eps {
        for y in [-0.5, 0.5] {
            let x = (d - y * s) / c;
            if x > -0.5 + eps && x < 0.5 - eps {
                out.push([x, y]);
            }
        }
    }
    out
}

const CORNERS: [[f64; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

/// Polygon (sorted) of the part of the pixel on the material side of the edge line.
fn clipped_pixel(angle: f64, d: f64) -> Vec<[f64; 2]> {
    let (s, c) = angle.sin_cos();
    let inside = |p: &[f64; 2]| p[0] * c + p[1] * s - d > -1e-9;
    let mut v = line_box_intersections(angle, d);
    v.extend(CORNERS.iter().filter(|p| inside(p)).copied());
    if v.len() < 3 {
        return v;
    }
    let (mx, my) = v.iter().fold((0.0, 0.0), |a, p| (a.0 + p[0], a.1 + p[1]));
    let (mx, my) = (mx / v.len() as f64, my / v.len() as f64);
    v.sort_by(|a, b| (a[1] - my).atan2(a[0] - mx).total_cmp(&(b[1] - my).atan2(b[0] - mx)));
    v
}

fn area_for_distance(d: f64, angle: f64) -> f64 {
    let v = clipped_pixel(angle, d);
    if v.len() < 3 {
        // Degenerate: the line misses the pixel, which is then fully inside or outside.
        return if -d > -1e-9 { 1.0 } else { 0.0 };
    }
    let mut a2 = 0.0;
    for i in 0..v.len() {
        let (p, q) = (v[i], v[(i + 1) % v.len()]);
        a2 += p[0] * q[1] - q[0] * p[1];
    }
    (a2 * 0.5).abs()
}

/// Inverse of [`area_for_distance`]: edge distance that leaves `area` of the pixel inside.
fn distance_for_area(area: f64, angle: f64) -> f64 {
    let max_d = 0.5 * (angle.cos().abs() + angle.sin().abs());
    if area <= 1e-9 {
        return max_d;
    }
    if area >= 1.0 - 1e-9 {
        return -max_d;
    }
    let (mut lo, mut hi) = (-max_d, max_d);
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        if area_for_distance(mid, angle) > area {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Estimates the material edge from a 3×3 coverage neighbourhood (row-major, center = 4)
/// with a Sobel gradient and returns the full pixel geometry.
pub fn pixel_geometry(grid: &[f64; 9], area: f64) -> PixelGeometry {
    if area >= 0.9999 {
        return PixelGeometry::full();
    }
    if area < 1e-9 {
        return PixelGeometry::default();
    }
    let gx = (grid[2] + 2.0 * grid[5] + grid[8]) - (grid[0] + 2.0 * grid[3] + grid[6]);
    let gy = (grid[6] + 2.0 * grid[7] + grid[8]) - (grid[0] + 2.0 * grid[1] + grid[2]);
    let angle = if gx.abs() + gy.abs() < 1e-9 { std::f64::consts::FRAC_PI_2 } else { gy.atan2(gx) };
    let distance = distance_for_area(area, angle);

    let (s, c) = angle.sin_cos();
    let inside = |p: [f64; 2]| p[0] * c + p[1] * s - distance > -1e-9;
    let hits = line_box_intersections(angle, distance);
    // Pixel edges: up (y=-0.5), down (y=+0.5), left (x=-0.5), right (x=+0.5).
    let edges: [[[f64; 2]; 2]; 4] = [
        [[-0.5, -0.5], [0.5, -0.5]],
        [[-0.5, 0.5], [0.5, 0.5]],
        [[-0.5, -0.5], [-0.5, 0.5]],
        [[0.5, -0.5], [0.5, 0.5]],
    ];
    let mut edge_len = [0.0; 4];
    for (k, e) in edges.iter().enumerate() {
        let horizontal = k < 2;
        let mut coords: Vec<f64> = Vec::with_capacity(4);
        for p in e {
            if inside(*p) {
                coords.push(if horizontal { p[0] } else { p[1] });
            }
        }
        for h in &hits {
            let on = if horizontal { (h[1] - e[0][1]).abs() < 1e-9 } else { (h[0] - e[0][0]).abs() < 1e-9 };
            if on {
                coords.push(if horizontal { h[0] } else { h[1] });
            }
        }
        if coords.len() >= 2 {
            let lo = coords.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = coords.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            edge_len[k] = hi - lo;
        }
    }
    PixelGeometry { area, angle, distance, edge_len }
}

/// Pixel classification used by the result views.
pub mod material {
    pub const VOID: u8 = 0;
    pub const CONDUCTOR: u8 = 1;
    /// Conductor without connection to plus/minus (ignored in the simulation).
    pub const ISOLATED: u8 = 2;
    /// Terminal pixels are `TERMINAL + terminal id`.
    pub const TERMINAL: u8 = 10;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Neighbor {
    None,
    Node(u32),
    Terminal(u32),
}

/// Resistor network ready to be solved.
pub struct Network {
    pub nx: usize,
    pub ny: usize,
    /// Pixel index of every active conductor node.
    pub node_pixel: Vec<u32>,
    pub node_area: Vec<f32>,
    /// Edge conductances (up, down, left, right) in units of 1/R□.
    pub g: Vec<[f64; 4]>,
    pub nb: Vec<[Neighbor; 4]>,
    /// Terminal id per pixel (or -1).
    pub term_of_pixel: Vec<i32>,
    /// Combined terminal coverage per pixel.
    pub term_cov: Vec<f32>,
    pub material: Vec<u8>,
    pub num_terminals: usize,
    pub source: usize,
    pub sink: usize,
    /// Unknown index of each floating terminal that is part of the system.
    pub floating_index: Vec<Option<usize>>,
    pub total_conductor_nodes: usize,
    pub removed_nodes: usize,
    pub warnings: Vec<String>,
}

impl Network {
    pub fn num_unknowns(&self) -> usize {
        self.node_pixel.len() + self.floating_index.iter().filter(|f| f.is_some()).count()
    }

    pub fn build(raster: &Raster, source: usize, sink: usize, terminal_names: &[String]) -> Result<Network> {
        let (nx, ny) = (raster.grid.nx, raster.grid.ny);
        let n_pix = nx * ny;
        let nt = raster.terminals.len();
        if source >= nt || sink >= nt {
            return Err(CrazeError::Setup("Plus- oder Minus-Terminal fehlt.".into()));
        }

        // --- Classify pixels ---------------------------------------------------------
        let mut term_of_pixel = vec![-1i32; n_pix];
        let mut term_cov = vec![0f32; n_pix];
        let mut cond = vec![0f32; n_pix];
        term_of_pixel
            .par_iter_mut()
            .zip(term_cov.par_iter_mut())
            .zip(cond.par_iter_mut())
            .enumerate()
            .for_each(|(i, ((tp, tc), c))| {
                let mut sum = 0.0f32;
                let mut best = (-1i32, 0.0f32);
                for (t, layer) in raster.terminals.iter().enumerate() {
                    let v = layer[i];
                    sum += v;
                    if v > best.1 {
                        best = (t as i32, v);
                    }
                }
                let t = sum.min(1.0);
                if t as f64 > EPS_AREA {
                    *tp = best.0;
                    *tc = t;
                }
                *c = raster.conductor[i].min(1.0 - t).max(0.0);
            });

        let cov_at = |buf: &[f32], x: i64, y: i64| -> f64 {
            if x < 0 || y < 0 || x >= nx as i64 || y >= ny as i64 { 0.0 } else { buf[y as usize * nx + x as usize] as f64 }
        };

        // Terminals touching each other would be a short circuit.
        for y in 0..ny {
            for x in 0..nx {
                let t = term_of_pixel[y * nx + x];
                if t < 0 {
                    continue;
                }
                for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                    let (x2, y2) = (x + dx, y + dy);
                    if x2 < nx && y2 < ny {
                        let t2 = term_of_pixel[y2 * nx + x2];
                        if t2 >= 0 && t2 != t {
                            let name = |i: i32| terminal_names.get(i as usize).cloned().unwrap_or_default();
                            return Err(CrazeError::Setup(format!(
                                "Die Terminals „{}“ und „{}“ berühren sich (Kurzschluss). Bitte die Kontaktflächen trennen.",
                                name(t),
                                name(t2)
                            )));
                        }
                    }
                }
            }
        }

        // Conductor nodes (pixels with material but without terminal).
        let mut node_of_pixel = vec![-1i32; n_pix];
        let mut node_pixel = Vec::new();
        for i in 0..n_pix {
            if term_of_pixel[i] < 0 && cond[i] as f64 > EPS_AREA {
                node_of_pixel[i] = node_pixel.len() as i32;
                node_pixel.push(i as u32);
            }
        }
        let total_conductor_nodes = node_pixel.len();
        if total_conductor_nodes == 0 {
            return Err(CrazeError::Setup("Auf den Leiter-Layern wurde kein Material gefunden.".into()));
        }

        // --- Sub-pixel geometry --------------------------------------------------------
        // Edge direction of conductor pixels is estimated from the combined coverage of
        // conductor and terminals, so pixels next to a contact see the real material edge.
        let combined: Vec<f32> = cond.iter().zip(&term_cov).map(|(c, t)| (c + t).min(1.0)).collect();
        let grid9 = |buf: &[f32], x: i64, y: i64| -> [f64; 9] {
            let mut g = [0.0; 9];
            let mut k = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    g[k] = cov_at(buf, x + dx, y + dy);
                    k += 1;
                }
            }
            g
        };
        let node_geo: Vec<PixelGeometry> = node_pixel
            .par_iter()
            .map(|&p| {
                let (x, y) = ((p as usize % nx) as i64, (p as usize / nx) as i64);
                let a = cond[p as usize] as f64;
                let a = if a >= 0.999 { 1.0 } else { a };
                pixel_geometry(&grid9(&combined, x, y), a)
            })
            .collect();
        let term_geo = |p: usize| -> PixelGeometry {
            let (x, y) = ((p % nx) as i64, (p / nx) as i64);
            pixel_geometry(&grid9(&term_cov, x, y), term_cov[p] as f64)
        };

        // --- Conductances ----------------------------------------------------------------
        let mut g = vec![[0.0f64; 4]; node_pixel.len()];
        let mut nb = vec![[Neighbor::None; 4]; node_pixel.len()];
        g.par_iter_mut().zip(nb.par_iter_mut()).enumerate().for_each(|(j, (gj, nbj))| {
            let p = node_pixel[j] as usize;
            let (x, y) = ((p % nx) as i64, (p / nx) as i64);
            let geo = &node_geo[j];
            for (k, (dx, dy)) in DIRS.iter().enumerate() {
                let (xn, yn) = (x + dx, y + dy);
                if xn < 0 || yn < 0 || xn >= nx as i64 || yn >= ny as i64 {
                    continue;
                }
                let q = yn as usize * nx + xn as usize;
                if node_of_pixel[q] >= 0 {
                    let m = node_of_pixel[q] as usize;
                    let a_eff = 0.5 * (geo.edge_len[k] + node_geo[m].edge_len[OPPOSITE[k]]);
                    if a_eff > 1e-9 {
                        gj[k] = a_eff; // center distance is one pixel
                        nbj[k] = Neighbor::Node(m as u32);
                    }
                } else if term_of_pixel[q] >= 0 {
                    let tg = term_geo(q);
                    let (a_eff, l_eff) = if tg.area > 0.999 {
                        (0.5 * (geo.edge_len[k] + 1.0), 0.5)
                    } else {
                        // Contact interface is a straight line inside the terminal pixel.
                        let edge_angle = if k < 2 { 0.0 } else { std::f64::consts::FRAC_PI_2 };
                        let iface_angle = tg.angle + std::f64::consts::FRAC_PI_2;
                        let a = geo.edge_len[k] * (edge_angle - iface_angle).cos().abs();
                        // Distance from this node's center to the interface line (terminal-local coords).
                        let (s, c) = tg.angle.sin_cos();
                        let (ox, oy) = (-(*dx as f64), -(*dy as f64));
                        let l = (ox * c + oy * s - tg.distance).abs().max(1e-4);
                        (a, l)
                    };
                    if a_eff > 1e-9 {
                        gj[k] = a_eff / l_eff;
                        nbj[k] = Neighbor::Terminal(term_of_pixel[q] as u32);
                    }
                }
            }
        });

        // --- Connectivity: keep nodes connected to plus or minus ---------------------------
        let n = node_pixel.len();
        let mut reached = vec![false; n];
        let mut term_reached = vec![false; nt];
        let mut from_source = vec![false; nt];
        // BFS over nodes; terminals act as super nodes (all their pixels are connected).
        let mut term_nodes: Vec<Vec<u32>> = vec![Vec::new(); nt];
        for j in 0..n {
            for k in 0..4 {
                if let Neighbor::Terminal(t) = nb[j][k] {
                    term_nodes[t as usize].push(j as u32);
                }
            }
        }
        let bfs = |start: usize, reached: &mut Vec<bool>, term_reached: &mut Vec<bool>| {
            let mut stack: Vec<u32> = Vec::new();
            let mut tstack = vec![start];
            term_reached[start] = true;
            loop {
                if let Some(t) = tstack.pop() {
                    for &j in &term_nodes[t] {
                        if !reached[j as usize] {
                            reached[j as usize] = true;
                            stack.push(j);
                        }
                    }
                } else if let Some(j) = stack.pop() {
                    for k in 0..4 {
                        match nb[j as usize][k] {
                            Neighbor::Node(m) if !reached[m as usize] => {
                                reached[m as usize] = true;
                                stack.push(m);
                            }
                            Neighbor::Terminal(t) if !term_reached[t as usize] => {
                                term_reached[t as usize] = true;
                                tstack.push(t as usize);
                            }
                            _ => {}
                        }
                    }
                } else {
                    break;
                }
            }
        };
        bfs(source, &mut reached, &mut term_reached);
        from_source.copy_from_slice(&term_reached);
        if !from_source[sink] {
            return Err(CrazeError::Setup(format!(
                "Zwischen „{}“ und „{}“ besteht keine leitende Verbindung. Berühren beide Terminals die Leiterbahn?",
                terminal_names.get(source).cloned().unwrap_or_default(),
                terminal_names.get(sink).cloned().unwrap_or_default()
            )));
        }
        // Parts hanging only on the minus terminal are solvable as well (they sit at 0 V).
        bfs(sink, &mut reached, &mut term_reached);

        // Compact the node list to the reached nodes.
        let mut remap = vec![u32::MAX; n];
        let mut new_pixel = Vec::new();
        for j in 0..n {
            if reached[j] {
                remap[j] = new_pixel.len() as u32;
                new_pixel.push(node_pixel[j]);
            }
        }
        let removed_nodes = n - new_pixel.len();
        let mut g2 = Vec::with_capacity(new_pixel.len());
        let mut nb2 = Vec::with_capacity(new_pixel.len());
        let mut area2 = Vec::with_capacity(new_pixel.len());
        for j in 0..n {
            if !reached[j] {
                continue;
            }
            let mut row = nb[j];
            for r in row.iter_mut() {
                if let Neighbor::Node(m) = *r {
                    *r = Neighbor::Node(remap[m as usize]);
                }
            }
            g2.push(g[j]);
            nb2.push(row);
            area2.push(node_geo[j].area as f32);
        }

        let mut material = vec![material::VOID; n_pix];
        for i in 0..n_pix {
            if term_of_pixel[i] >= 0 {
                material[i] = material::TERMINAL + term_of_pixel[i] as u8;
            } else if node_of_pixel[i] >= 0 {
                material[i] =
                    if reached[node_of_pixel[i] as usize] { material::CONDUCTOR } else { material::ISOLATED };
            }
        }

        let mut floating_index = vec![None; nt];
        let mut next = new_pixel.len();
        for t in 0..nt {
            if t != source && t != sink && term_reached[t] {
                floating_index[t] = Some(next);
                next += 1;
            }
        }

        let mut warnings = Vec::new();
        if removed_nodes > 0 {
            let pct = 100.0 * removed_nodes as f64 / total_conductor_nodes as f64;
            warnings.push(format!(
                "{removed_nodes} Pixel ({pct:.1} %) Leitermaterial haben keine Verbindung zu einem Terminal und werden ignoriert."
            ));
        }

        Ok(Network {
            nx,
            ny,
            node_pixel: new_pixel,
            node_area: area2,
            g: g2,
            nb: nb2,
            term_of_pixel,
            term_cov,
            material,
            num_terminals: nt,
            source,
            sink,
            floating_index,
            total_conductor_nodes,
            removed_nodes,
            warnings,
        })
    }

    /// Assembles the symmetric positive definite system A·x = b for
    /// V(source) = 1, V(sink) = 0 and floating terminals as unknowns.
    pub fn assemble(&self) -> (Csr, Vec<f64>) {
        let n_nodes = self.node_pixel.len();
        let n = self.num_unknowns();
        let fixed_v = |t: usize| -> Option<f64> {
            if t == self.source {
                Some(1.0)
            } else if t == self.sink {
                Some(0.0)
            } else {
                None
            }
        };
        let mut b = vec![0.0; n];
        let mut indptr = Vec::with_capacity(n + 1);
        let mut indices: Vec<u32> = Vec::with_capacity(n_nodes * 5);
        let mut vals: Vec<f64> = Vec::with_capacity(n_nodes * 5);
        indptr.push(0usize);
        let mut float_rows: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n - n_nodes];
        let mut row: Vec<(u32, f64)> = Vec::with_capacity(8);
        for j in 0..n_nodes {
            row.clear();
            let mut diag = 0.0;
            for k in 0..4 {
                let gk = self.g[j][k];
                if gk <= 0.0 {
                    continue;
                }
                match self.nb[j][k] {
                    Neighbor::Node(m) => {
                        diag += gk;
                        row.push((m, -gk));
                    }
                    Neighbor::Terminal(t) => {
                        diag += gk;
                        match fixed_v(t as usize) {
                            Some(v) => b[j] += gk * v,
                            None => {
                                let f = self.floating_index[t as usize].expect("reached floating terminal");
                                row.push((f as u32, -gk));
                                let fr = &mut float_rows[f - n_nodes];
                                fr.push((j as u32, -gk));
                                fr.push((f as u32, gk));
                            }
                        }
                    }
                    Neighbor::None => {}
                }
            }
            row.push((j as u32, diag));
            row.sort_unstable_by_key(|e| e.0);
            for &(c, v) in row.iter() {
                if indices.len() > indptr[j] && *indices.last().unwrap() == c {
                    *vals.last_mut().unwrap() += v;
                } else {
                    indices.push(c);
                    vals.push(v);
                }
            }
            indptr.push(indices.len());
        }
        let tail = Csr::from_rows(n - n_nodes, float_rows);
        let base = indices.len();
        indices.extend_from_slice(&tail.indices);
        vals.extend_from_slice(&tail.vals);
        indptr.extend(tail.indptr[1..].iter().map(|p| p + base));
        (Csr { n, indptr, indices, vals }, b)
    }

    /// Potentials of all terminals for a solution vector.
    pub fn terminal_potentials(&self, x: &[f64]) -> Vec<f64> {
        (0..self.num_terminals)
            .map(|t| {
                if t == self.source {
                    1.0
                } else if t == self.sink {
                    0.0
                } else {
                    self.floating_index[t].map(|f| x[f]).unwrap_or(f64::NAN)
                }
            })
            .collect()
    }
}
