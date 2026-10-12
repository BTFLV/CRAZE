//! Material library and layer stack → sheet resistance R□(T).

use serde::{Deserialize, Serialize};

/// Reference temperature of the tabulated resistivities in °C.
pub const T_REF_C: f64 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub name: String,
    /// Resistivity at 20 °C in Ω·m.
    pub resistivity: f64,
    /// Linear temperature coefficient α in 1/K.
    pub tcr: f64,
}

/// Bulk values at 20 °C. Thin films usually have a higher resistivity and a lower TCR;
/// for accurate results enter a measured sheet resistance instead.
pub fn library() -> Vec<Material> {
    let m = |name: &str, resistivity: f64, tcr: f64| Material { name: name.into(), resistivity, tcr };
    vec![
        m("Aluminium", 2.82e-8, 3.9e-3),
        m("Chrom", 1.25e-7, 3.0e-3),
        m("Gold", 2.44e-8, 3.4e-3),
        m("Kupfer", 1.68e-8, 4.04e-3),
        m("Molybdän", 5.34e-8, 4.35e-3),
        m("Nickel", 6.99e-8, 6.0e-3),
        m("NiCr 80/20", 1.10e-6, 0.4e-3),
        m("Platin", 1.06e-7, 3.92e-3),
        m("Silber", 1.59e-8, 3.8e-3),
        m("Tantal", 1.31e-7, 3.5e-3),
        m("Titan", 4.20e-7, 3.8e-3),
        m("Wolfram", 5.60e-8, 4.5e-3),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackLayer {
    pub material: Material,
    pub thickness_nm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Stack {
    /// Layers conduct in parallel.
    Layers { layers: Vec<StackLayer> },
    /// Measured sheet resistance at `t_ref_c` with an effective TCR.
    Measured { sheet_resistance: f64, tcr: f64, t_ref_c: f64 },
}

impl Default for Stack {
    fn default() -> Self {
        let lib = library();
        let get = |n: &str| lib.iter().find(|m| m.name == n).unwrap().clone();
        Stack::Layers {
            layers: vec![
                StackLayer { material: get("Platin"), thickness_nm: 100.0 },
                StackLayer { material: get("Tantal"), thickness_nm: 10.0 },
            ],
        }
    }
}

impl Stack {
    /// Sheet resistance in Ω/□ at temperature `t_c` (°C).
    pub fn sheet_resistance(&self, t_c: f64) -> f64 {
        match self {
            Stack::Layers { layers } => {
                let g: f64 = layers
                    .iter()
                    .filter(|l| l.thickness_nm > 0.0 && l.material.resistivity > 0.0)
                    .map(|l| {
                        let rho = l.material.resistivity * (1.0 + l.material.tcr * (t_c - T_REF_C));
                        l.thickness_nm * 1e-9 / rho
                    })
                    .sum();
                if g > 0.0 { 1.0 / g } else { f64::INFINITY }
            }
            Stack::Measured { sheet_resistance, tcr, t_ref_c } => sheet_resistance * (1.0 + tcr * (t_c - t_ref_c)),
        }
    }

    pub fn total_thickness_m(&self) -> Option<f64> {
        match self {
            Stack::Layers { layers } => Some(layers.iter().map(|l| l.thickness_nm * 1e-9).sum()),
            Stack::Measured { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_layers() {
        let s = Stack::default();
        // Pt 100 nm: 1.06 Ω/□, Ta 10 nm: 13.1 Ω/□ → parallel ≈ 0.9807 Ω/□
        let r = s.sheet_resistance(20.0);
        let expect = 1.0 / (1.0 / 1.06 + 1.0 / 13.1);
        assert!((r - expect).abs() < 1e-9, "{r}");
        assert!(s.sheet_resistance(100.0) > r);
    }
}
