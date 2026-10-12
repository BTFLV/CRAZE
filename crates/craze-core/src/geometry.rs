//! Basic 2D geometry: points, affine transforms, path expansion and polygon helpers.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn dot(self, o: Pt) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: Pt) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> Pt {
        let l = self.len();
        if l > 0.0 { self * (1.0 / l) } else { self }
    }
    /// Left-hand normal (rotated +90°).
    pub fn perp(self) -> Pt {
        Pt::new(-self.y, self.x)
    }
}

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, s: f64) -> Pt {
        Pt::new(self.x * s, self.y * s)
    }
}
impl std::ops::Neg for Pt {
    type Output = Pt;
    fn neg(self) -> Pt {
        Pt::new(-self.x, -self.y)
    }
}
impl std::ops::Div<f64> for Pt {
    type Output = Pt;
    fn div(self, s: f64) -> Pt {
        Pt::new(self.x / s, self.y / s)
    }
}

/// 2D affine transform: x' = a·x + b·y + tx, y' = c·x + d·y + ty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn scale(s: f64) -> Self {
        Affine { a: s, d: s, ..Self::IDENTITY }
    }

    /// GDSII placement: reflect about x-axis (optional), magnify, rotate, then translate.
    pub fn from_strans(origin: Pt, angle_deg: f64, mag: f64, reflect: bool) -> Self {
        let (s, c) = angle_deg.to_radians().sin_cos();
        // Snap tiny values for exact 90° multiples.
        let snap = |v: f64| if v.abs() < 1e-15 { 0.0 } else { v };
        let (s, c) = (snap(s), snap(c));
        let f = if reflect { -1.0 } else { 1.0 };
        Affine { a: mag * c, b: -mag * s * f, c: mag * s, d: mag * c * f, tx: origin.x, ty: origin.y }
    }

    pub fn apply(&self, p: Pt) -> Pt {
        Pt::new(self.a * p.x + self.b * p.y + self.tx, self.c * p.x + self.d * p.y + self.ty)
    }

    /// Composition: first `inner`, then `self`.
    pub fn then(&self, inner: &Affine) -> Affine {
        Affine {
            a: self.a * inner.a + self.b * inner.c,
            b: self.a * inner.b + self.b * inner.d,
            c: self.c * inner.a + self.d * inner.c,
            d: self.c * inner.b + self.d * inner.d,
            tx: self.a * inner.tx + self.b * inner.ty + self.tx,
            ty: self.c * inner.tx + self.d * inner.ty + self.ty,
        }
    }

    /// Isotropic scale factor of the transform.
    pub fn mag(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

/// Signed polygon area (positive for counter-clockwise in a y-up system).
pub fn signed_area(poly: &[Pt]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        a += poly[i].cross(poly[(i + 1) % n]);
    }
    0.5 * a
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: Pt,
    pub max: Pt,
}

impl BBox {
    pub fn empty() -> Self {
        BBox { min: Pt::new(f64::INFINITY, f64::INFINITY), max: Pt::new(f64::NEG_INFINITY, f64::NEG_INFINITY) }
    }
    pub fn add(&mut self, p: Pt) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }
    pub fn union(&mut self, o: &BBox) {
        if o.is_valid() {
            self.add(o.min);
            self.add(o.max);
        }
    }
    pub fn is_valid(&self) -> bool {
        self.max.x > self.min.x && self.max.y > self.min.y
    }
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
    pub fn of(poly: &[Pt]) -> Self {
        let mut b = Self::empty();
        for p in poly {
            b.add(*p);
        }
        b
    }
}

fn dedup_points(pts: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last().is_none_or(|q| (*q - p).len() > 1e-9) {
            out.push(p);
        }
    }
    out
}

/// Expands a GDSII path into a closed outline polygon.
///
/// * `pathtype` 0: flush ends, 1: round ends, 2: square ends (extended by half width),
///   4: custom extensions `bgnextn` / `endextn`.
/// * Joins are mitered, matching the GDSII definition used by layout editors.
pub fn path_to_polygon(pts: &[Pt], width: f64, pathtype: i16, bgnextn: f64, endextn: f64) -> Vec<Pt> {
    let mut pts = dedup_points(pts);
    let hw = width.abs() * 0.5;
    if pts.len() < 2 || hw <= 0.0 {
        return vec![];
    }
    let n = pts.len();
    let (ext_b, ext_e) = match pathtype {
        2 => (hw, hw),
        4 => (bgnextn, endextn),
        _ => (0.0, 0.0),
    };
    let d0 = (pts[1] - pts[0]).normalized();
    let dn = (pts[n - 1] - pts[n - 2]).normalized();
    pts[0] = pts[0] - d0 * ext_b;
    pts[n - 1] = pts[n - 1] + dn * ext_e;

    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        if i == 0 || i == n - 1 {
            let d = if i == 0 { d0 } else { dn };
            let nrm = d.perp();
            left.push(pts[i] + nrm * hw);
            right.push(pts[i] - nrm * hw);
            continue;
        }
        let d1 = (pts[i] - pts[i - 1]).normalized();
        let d2 = (pts[i + 1] - pts[i]).normalized();
        let n1 = d1.perp();
        let n2 = d2.perp();
        let denom = 1.0 + n1.dot(n2);
        if denom.abs() < 1e-9 {
            // 180° reversal: no meaningful miter, use the plain offsets.
            left.push(pts[i] + n1 * hw);
            right.push(pts[i] - n1 * hw);
        } else {
            // Miter vector: offset along the bisector of both normals.
            let m = (n1 + n2) * (hw / denom);
            left.push(pts[i] + m);
            right.push(pts[i] - m);
        }
    }

    let mut poly = Vec::with_capacity(2 * n + 34);
    if pathtype == 1 {
        // Round caps: half circles at both ends.
        const SEG: usize = 16;
        poly.extend_from_slice(&left);
        let base = dn.perp();
        for k in 1..SEG {
            let a = std::f64::consts::PI * k as f64 / SEG as f64;
            let dir = base * a.cos() + dn * a.sin();
            poly.push(pts[n - 1] + dir * hw);
        }
        poly.extend(right.iter().rev().copied());
        let base0 = (-d0).perp();
        for k in 1..SEG {
            let a = std::f64::consts::PI * k as f64 / SEG as f64;
            let dir = base0 * a.cos() + d0 * (-a.sin());
            poly.push(pts[0] + dir * hw);
        }
    } else {
        poly.extend_from_slice(&left);
        poly.extend(right.iter().rev().copied());
    }
    poly
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_area_flush_and_square() {
        let pts = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(10.0, 10.0)];
        let a0 = signed_area(&path_to_polygon(&pts, 2.0, 0, 0.0, 0.0)).abs();
        // Two 10x2 legs with a mitered 90° corner: 10*2 + 10*2 = 40 (corner square counted once).
        assert!((a0 - 40.0).abs() < 1e-9, "{a0}");
        let a2 = signed_area(&path_to_polygon(&pts, 2.0, 2, 0.0, 0.0)).abs();
        assert!((a2 - 44.0).abs() < 1e-9, "{a2}");
    }

    #[test]
    fn round_caps_area() {
        let pts = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0)];
        let a = signed_area(&path_to_polygon(&pts, 2.0, 1, 0.0, 0.0)).abs();
        let exact = 20.0 + std::f64::consts::PI;
        assert!((a - exact).abs() < 0.05, "{a}");
    }

    #[test]
    fn strans_rotation_reflection() {
        let t = Affine::from_strans(Pt::new(1.0, 2.0), 90.0, 2.0, true);
        // (1,0) -> reflect (1,0) -> mag (2,0) -> rot90 (0,2) -> +origin (1,4)
        let p = t.apply(Pt::new(1.0, 0.0));
        assert!((p.x - 1.0).abs() < 1e-12 && (p.y - 4.0).abs() < 1e-12, "{p:?}");
        // (0,1) -> reflect (0,-1) -> mag (0,-2) -> rot90 (2,0) -> (3,2)
        let q = t.apply(Pt::new(0.0, 1.0));
        assert!((q.x - 3.0).abs() < 1e-12 && (q.y - 2.0).abs() < 1e-12, "{q:?}");
    }
}
