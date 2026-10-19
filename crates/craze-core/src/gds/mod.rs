//! GDSII library model and hierarchy flattening.

pub mod records;

use std::collections::{BTreeMap, HashMap, HashSet};

use records::{Record, RecordData, rt};
use serde::{Deserialize, Serialize};

use crate::error::{CrazeError, Result};
use crate::geometry::{Affine, Pt, path_to_polygon};

/// Layer/datatype pair identifying a GDS layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LayerKey {
    pub layer: i16,
    pub datatype: i16,
}

impl std::fmt::Display for LayerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.layer, self.datatype)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Strans {
    pub reflect: bool,
    pub mag: f64,
    pub angle_deg: f64,
}

impl Strans {
    fn affine(&self, origin: Pt) -> Affine {
        Affine::from_strans(origin, self.angle_deg, self.mag, self.reflect)
    }
}

#[derive(Debug, Clone)]
pub enum Element {
    Boundary { key: LayerKey, xy: Vec<Pt> },
    Path { key: LayerKey, pathtype: i16, width: f64, bgnextn: f64, endextn: f64, xy: Vec<Pt>, record_index: Option<usize> },
    Box { key: LayerKey, xy: Vec<Pt> },
    Text { key: LayerKey, text: String, pos: Pt },
    SRef { name: String, trans: Strans, origin: Pt },
    ARef { name: String, trans: Strans, cols: i32, rows: i32, xy: [Pt; 3] },
}

#[derive(Debug, Clone)]
pub struct Structure {
    pub name: String,
    pub elements: Vec<Element>,
}

/// A parsed GDSII library. Keeps the raw record stream so it can be written back
/// with modified path widths (used by the optimizer).
#[derive(Debug, Clone)]
pub struct Library {
    pub name: String,
    /// Size of one database unit in meters.
    pub db_unit_m: f64,
    /// Size of one user unit in database units (usually 1e-3 → user unit = µm).
    pub user_unit: f64,
    pub structures: Vec<Structure>,
    pub records: Vec<Record>,
}

impl Library {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let records = if records::is_binary(bytes) {
            records::read_binary(bytes)?
        } else {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| CrazeError::Gds("the file is neither binary GDSII nor GDS text".into()))?;
            records::read_text(text)?
        };
        Self::from_records(records)
    }

    pub fn from_records(records: Vec<Record>) -> Result<Self> {
        let mut name = String::new();
        let mut db_unit_m = 1e-9;
        let mut user_unit = 1e-3;
        let mut structures = Vec::new();
        let mut cur: Option<Structure> = None;

        // Element under construction.
        #[derive(Default)]
        struct El {
            kind: u8,
            key: LayerKey,
            pathtype: i16,
            width: f64,
            bgnextn: f64,
            endextn: f64,
            xy: Vec<Pt>,
            sname: String,
            strans: Strans,
            colrow: (i32, i32),
            text: String,
            width_idx: Option<usize>,
        }
        let mut el: Option<El> = None;

        for (i, r) in records.iter().enumerate() {
            match r.rtype {
                rt::LIBNAME => name = r.text().to_string(),
                rt::UNITS => {
                    let v = r.reals();
                    if v.len() >= 2 && v[1] > 0.0 {
                        user_unit = v[0];
                        db_unit_m = v[1];
                    }
                }
                rt::BGNSTR => cur = Some(Structure { name: String::new(), elements: vec![] }),
                rt::STRNAME => {
                    if let Some(s) = cur.as_mut() {
                        s.name = r.text().to_string();
                    }
                }
                rt::ENDSTR => {
                    if let Some(s) = cur.take() {
                        structures.push(s);
                    }
                }
                rt::BOUNDARY | rt::PATH | rt::SREF | rt::AREF | rt::TEXT | rt::BOX | rt::NODE => {
                    el = Some(El {
                        kind: r.rtype,
                        strans: Strans { reflect: false, mag: 1.0, angle_deg: 0.0 },
                        colrow: (1, 1),
                        ..Default::default()
                    });
                }
                _ => {
                    let Some(e) = el.as_mut() else { continue };
                    match r.rtype {
                        rt::LAYER => e.key.layer = r.int() as i16,
                        rt::DATATYPE | rt::BOXTYPE | rt::TEXTTYPE => e.key.datatype = r.int() as i16,
                        rt::PATHTYPE => e.pathtype = r.int() as i16,
                        rt::WIDTH => {
                            e.width = r.int() as f64;
                            e.width_idx = Some(i);
                        }
                        rt::BGNEXTN => e.bgnextn = r.int() as f64,
                        rt::ENDEXTN => e.endextn = r.int() as f64,
                        rt::XY => {
                            e.xy = r.i32s().chunks_exact(2).map(|c| Pt::new(c[0] as f64, c[1] as f64)).collect();
                        }
                        rt::SNAME => e.sname = r.text().to_string(),
                        rt::STRING => e.text = r.text().to_string(),
                        rt::STRANS => e.strans.reflect = r.bits() & 0x8000 != 0,
                        rt::MAG => e.strans.mag = r.reals().first().copied().unwrap_or(1.0),
                        rt::ANGLE => e.strans.angle_deg = r.reals().first().copied().unwrap_or(0.0),
                        rt::COLROW => {
                            let v = r.i16s();
                            if v.len() >= 2 {
                                e.colrow = (v[0] as i32, v[1] as i32);
                            }
                        }
                        rt::ENDEL => {
                            let e = el.take().unwrap();
                            let element = match e.kind {
                                rt::BOUNDARY if e.xy.len() >= 3 => Some(Element::Boundary { key: e.key, xy: e.xy }),
                                rt::PATH if e.xy.len() >= 2 => Some(Element::Path {
                                    key: e.key,
                                    pathtype: e.pathtype,
                                    width: e.width,
                                    bgnextn: e.bgnextn,
                                    endextn: e.endextn,
                                    xy: e.xy,
                                    record_index: e.width_idx,
                                }),
                                rt::BOX if e.xy.len() >= 4 => Some(Element::Box { key: e.key, xy: e.xy }),
                                rt::TEXT if !e.xy.is_empty() => Some(Element::Text { key: e.key, text: e.text, pos: e.xy[0] }),
                                rt::SREF if !e.xy.is_empty() => {
                                    Some(Element::SRef { name: e.sname, trans: e.strans, origin: e.xy[0] })
                                }
                                rt::AREF if e.xy.len() >= 3 => Some(Element::ARef {
                                    name: e.sname,
                                    trans: e.strans,
                                    cols: e.colrow.0.max(1),
                                    rows: e.colrow.1.max(1),
                                    xy: [e.xy[0], e.xy[1], e.xy[2]],
                                }),
                                _ => None,
                            };
                            if let (Some(element), Some(s)) = (element, cur.as_mut()) {
                                s.elements.push(element);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if structures.is_empty() {
            return Err(CrazeError::Gds("no cells (structures) found in the file".into()));
        }
        Ok(Library { name, db_unit_m, user_unit, structures, records })
    }

    /// Structures that are not referenced by any other structure.
    pub fn top_cells(&self) -> Vec<String> {
        let referenced: HashSet<&str> = self
            .structures
            .iter()
            .flat_map(|s| s.elements.iter())
            .filter_map(|e| match e {
                Element::SRef { name, .. } | Element::ARef { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        self.structures.iter().filter(|s| !referenced.contains(s.name.as_str())).map(|s| s.name.clone()).collect()
    }

    /// Flattens the hierarchy below `top` into polygons in micrometers.
    pub fn flatten(&self, top: &str) -> Result<FlatLayout> {
        let index: HashMap<&str, &Structure> = self.structures.iter().map(|s| (s.name.as_str(), s)).collect();
        let root = index.get(top).ok_or_else(|| CrazeError::Gds(format!("cell '{top}' not found")))?;
        let to_um = self.db_unit_m * 1e6;
        let mut out = FlatLayout { top_cell: top.to_string(), shapes: BTreeMap::new(), labels: vec![] };
        let mut stack = Vec::new();
        flatten_rec(&index, root, &Affine::scale(to_um), &mut out, &mut stack, 0)?;
        Ok(out)
    }

    /// Returns a copy of the record stream in which the width of every path on one of
    /// `keys` is multiplied by `factor`. Used to write optimized layouts.
    pub fn with_scaled_path_widths(&self, keys: &HashSet<LayerKey>, factor: f64) -> Vec<Record> {
        let mut recs = self.records.clone();
        for s in &self.structures {
            for e in &s.elements {
                if let Element::Path { key, record_index: Some(idx), width, .. } = e {
                    if keys.contains(key) {
                        // Negative width means "absolute width" in GDSII; keep the sign.
                        let w = (width * factor).round() as i32;
                        recs[*idx].data = RecordData::I32(vec![w]);
                    }
                }
            }
        }
        recs
    }

    /// Number of path elements on the given layers (anywhere in the library).
    pub fn count_paths(&self, keys: &HashSet<LayerKey>) -> usize {
        self.structures
            .iter()
            .flat_map(|s| &s.elements)
            .filter(|e| matches!(e, Element::Path { key, record_index: Some(_), .. } if keys.contains(key)))
            .count()
    }
}

/// Origin of a flattened shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Polygon,
    Path,
    Box,
}

#[derive(Debug, Clone)]
pub struct FlatShape {
    pub kind: ShapeKind,
    /// Closed polygon outline in µm (no repeated last point).
    pub polygon: Vec<Pt>,
    /// Path width in µm (only for paths).
    pub path_width: f64,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub key: LayerKey,
    pub text: String,
    pub pos: Pt,
}

#[derive(Debug, Clone)]
pub struct FlatLayout {
    pub top_cell: String,
    pub shapes: BTreeMap<LayerKey, Vec<FlatShape>>,
    pub labels: Vec<Label>,
}

const MAX_DEPTH: usize = 64;

fn flatten_rec<'a>(
    index: &HashMap<&str, &'a Structure>,
    s: &'a Structure,
    t: &Affine,
    out: &mut FlatLayout,
    stack: &mut Vec<&'a str>,
    depth: usize,
) -> Result<()> {
    if depth > MAX_DEPTH || stack.contains(&s.name.as_str()) {
        return Err(CrazeError::Gds(format!("circular or too deeply nested cell reference at '{}'", s.name)));
    }
    stack.push(&s.name);
    for e in &s.elements {
        match e {
            Element::Boundary { key, xy } | Element::Box { key, xy } => {
                let mut poly: Vec<Pt> = xy.iter().map(|p| t.apply(*p)).collect();
                if poly.len() > 1 && poly.first() == poly.last() {
                    poly.pop();
                }
                if poly.len() >= 3 {
                    let kind = if matches!(e, Element::Box { .. }) { ShapeKind::Box } else { ShapeKind::Polygon };
                    out.shapes.entry(*key).or_default().push(FlatShape { kind, polygon: poly, path_width: 0.0 });
                }
            }
            Element::Path { key, pathtype, width, bgnextn, endextn, xy, .. } => {
                // Negative width: absolute, not affected by magnification.
                let local_w = if *width < 0.0 { width.abs() / t.mag() } else { *width };
                let poly = path_to_polygon(xy, local_w, *pathtype, *bgnextn, *endextn);
                if poly.len() >= 3 {
                    let polygon = poly.iter().map(|p| t.apply(*p)).collect();
                    out.shapes.entry(*key).or_default().push(FlatShape {
                        kind: ShapeKind::Path,
                        polygon,
                        path_width: local_w * t.mag(),
                    });
                }
            }
            Element::Text { key, text, pos } => {
                out.labels.push(Label { key: *key, text: text.clone(), pos: t.apply(*pos) });
            }
            Element::SRef { name, trans, origin } => {
                let child = index
                    .get(name.as_str())
                    .ok_or_else(|| CrazeError::Gds(format!("referenced cell '{name}' is missing")))?;
                let ct = t.then(&trans.affine(*origin));
                flatten_rec(index, child, &ct, out, stack, depth + 1)?;
            }
            Element::ARef { name, trans, cols, rows, xy } => {
                let child = index
                    .get(name.as_str())
                    .ok_or_else(|| CrazeError::Gds(format!("referenced cell '{name}' is missing")))?;
                let col_step = (xy[1] - xy[0]) / *cols as f64;
                let row_step = (xy[2] - xy[0]) / *rows as f64;
                for r in 0..*rows {
                    for c in 0..*cols {
                        let origin = xy[0] + col_step * c as f64 + row_step * r as f64;
                        let ct = t.then(&trans.affine(origin));
                        flatten_rec(index, child, &ct, out, stack, depth + 1)?;
                    }
                }
            }
        }
    }
    stack.pop();
    Ok(())
}
