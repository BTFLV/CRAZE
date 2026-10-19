# Changelog

## 1.0.0 — 2026-10-19

First public release.

### Simulation
- Resistance of thin-film layouts from GDSII: binary GDSII and KLayout GDS2 text, cell
  hierarchy with SREF/AREF, rotation, mirroring and magnification, all path types.
- Exact-area rasterization with a sub-pixel edge model; validated against analytic
  solutions (straight and rotated bars, Corbino ring, 90° corner) and KLayout.
- Any number of terminals: plus, minus and floating sense contacts for four-wire
  measurements.
- Sparse Cholesky solver, and conjugate gradients with IC(0) for very large problems.
- Material stack with temperature coefficients and R(T).

### App
- Six-step workflow: layout, layers, material, simulation, results, optimization.
- Potential, current density and power density with hotspot, current arrows,
  equipotential lines and cross-sections; resolution study with error estimate.
- Width optimizer that scales path widths to a target resistance and writes GDSII.
- Projects, PNG and HTML report export, English and German UI, light and dark appearance.
- macOS (universal) and Windows installers.

### Command line
- `craze info`, `simulate`, `sweep` and `optimize`.
