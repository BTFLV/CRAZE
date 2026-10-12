//! CRAZE core: GDSII import, rasterization, resistor-network construction and solving
//! for 2D thin-film resistance simulation. No GUI dependencies.

pub mod error;
pub mod gds;
pub mod geometry;
pub mod materials;
pub mod network;
pub mod optimize;
pub mod raster;
pub mod setup;
pub mod simulate;
pub mod solver;

pub use error::{CrazeError, Result};
