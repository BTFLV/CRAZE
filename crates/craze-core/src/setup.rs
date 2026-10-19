//! Simulation setup: which GDS layer plays which role, terminals and numerical settings.

use serde::{Deserialize, Serialize};

use crate::gds::{FlatLayout, LayerKey, ShapeKind};
use crate::geometry::signed_area;

/// Role of a GDS layer in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Role {
    /// Not used for the simulation.
    Ignore,
    /// Resistive thin film.
    Conductor,
    /// Ideal contact (equipotential) belonging to terminal `id`.
    Terminal { id: usize },
    /// Defines the simulated area (optional; otherwise derived from the geometry).
    Region,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerAssignment {
    pub key: LayerKey,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalDef {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SolverKind {
    /// Direct solver for typical sizes, iterative for very large problems.
    #[default]
    Auto,
    /// Sparse Cholesky factorization (exact up to rounding).
    Direct,
    /// Preconditioned conjugate gradients (low memory).
    Iterative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimSettings {
    /// Number of pixels along the longer side of the simulated area.
    pub resolution: u32,
    pub solver: SolverKind,
    /// Relative residual tolerance for the iterative solver.
    pub tolerance: f64,
    pub max_iterations: u32,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self { resolution: 1000, solver: SolverKind::Auto, tolerance: 1e-10, max_iterations: 200_000 }
    }
}

/// Complete description of what to simulate on a flattened layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    pub layers: Vec<LayerAssignment>,
    pub terminals: Vec<TerminalDef>,
    /// Terminal driven at the higher potential.
    pub source: usize,
    /// Terminal at 0 V.
    pub sink: usize,
}

impl Setup {
    pub fn role_of(&self, key: LayerKey) -> Role {
        self.layers.iter().find(|a| a.key == key).map(|a| a.role).unwrap_or(Role::Ignore)
    }

    pub fn keys_with(&self, pred: impl Fn(Role) -> bool) -> Vec<LayerKey> {
        self.layers.iter().filter(|a| pred(a.role)).map(|a| a.key).collect()
    }

    pub fn conductor_keys(&self) -> Vec<LayerKey> {
        self.keys_with(|r| r == Role::Conductor)
    }

    /// Checks the setup for problems the user can fix. Returns English messages.
    pub fn validate(&self, layout: &FlatLayout) -> Vec<String> {
        let mut errs = Vec::new();
        if self.conductor_keys().is_empty() {
            errs.push("No layer is assigned as conductor (resistive film).".into());
        }
        if self.terminals.len() < 2 {
            errs.push("At least two terminals are required.".into());
        }
        if self.source == self.sink {
            errs.push("Plus and minus terminal must be different.".into());
        }
        for (i, t) in self.terminals.iter().enumerate() {
            let has_shapes = self.layers.iter().any(|a| {
                a.role == Role::Terminal { id: i } && layout.shapes.get(&a.key).is_some_and(|s| !s.is_empty())
            });
            if !has_shapes && (i == self.source || i == self.sink) {
                errs.push(format!("Terminal “{}” has no geometry. Please assign a layer.", t.name));
            }
        }
        if self.source >= self.terminals.len() || self.sink >= self.terminals.len() {
            errs.push("Plus or minus terminal is invalid.".into());
        }
        errs
    }

    /// Suggests a role assignment for a freshly imported layout.
    ///
    /// The default CRAZE convention (1 = region, 2 = conductor, 3/4 = terminals) is
    /// recognized; otherwise the layer with the largest area becomes the conductor and the
    /// two smallest layers become terminals.
    pub fn suggest(layout: &FlatLayout) -> Setup {
        let keys: Vec<LayerKey> = layout.shapes.keys().copied().collect();
        let has = |l: i16| keys.iter().any(|k| k.layer == l);
        let terminals = vec![TerminalDef { name: "Plus".into() }, TerminalDef { name: "Minus".into() }];
        let mut layers: Vec<LayerAssignment> =
            keys.iter().map(|&key| LayerAssignment { key, role: Role::Ignore }).collect();

        if has(2) && has(3) && has(4) {
            for a in &mut layers {
                a.role = match a.key.layer {
                    1 => Role::Region,
                    2 => Role::Conductor,
                    3 => Role::Terminal { id: 0 },
                    4 => Role::Terminal { id: 1 },
                    _ => Role::Ignore,
                };
            }
        } else if keys.len() >= 3 {
            let area = |k: &LayerKey| -> f64 {
                layout.shapes[k].iter().map(|s| signed_area(&s.polygon).abs()).sum()
            };
            let mut by_area: Vec<(LayerKey, f64, bool)> = keys
                .iter()
                .map(|k| (*k, area(k), layout.shapes[k].iter().any(|s| s.kind == ShapeKind::Path)))
                .collect();
            by_area.sort_by(|a, b| a.1.total_cmp(&b.1));
            // Conductor: prefer layers with paths, then largest area.
            let cond = by_area
                .iter()
                .rev()
                .find(|e| e.2)
                .or(by_area.last())
                .map(|e| e.0)
                .unwrap();
            let small: Vec<LayerKey> = by_area.iter().map(|e| e.0).filter(|k| *k != cond).take(2).collect();
            for a in &mut layers {
                if a.key == cond {
                    a.role = Role::Conductor;
                } else if a.key == small[0] {
                    a.role = Role::Terminal { id: 0 };
                } else if a.key == small[1] {
                    a.role = Role::Terminal { id: 1 };
                }
            }
        } else if let Some(k) = keys.first() {
            layers.iter_mut().find(|a| a.key == *k).unwrap().role = Role::Conductor;
        }
        Setup { layers, terminals, source: 0, sink: 1 }
    }
}
