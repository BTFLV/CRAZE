# Numerics

CRAZE solves the steady-state current flow in a thin conducting sheet:

    ∇ · (σ_s ∇V) = 0          in the conductor
    V = 1 on the plus terminal, V = 0 on the minus terminal
    floating terminals: V = const, net current = 0
    n · ∇V = 0                 on all other edges (insulating)

with a uniform sheet conductance σ_s = 1 / R□. Because σ_s is uniform it drops out of the
geometry problem. CRAZE solves with σ_s = 1 and ΔV = 1, so the total current *I* gives the
number of squares N = 1 / I, and the resistance is R = N · R□ for any material and
temperature.

This document follows the pipeline in `crates/craze-core/src`.

## 1. Geometry import (`gds`, `geometry`)

- Binary GDSII and KLayout GDS2 text are parsed into the same record list.
- Hierarchy is flattened recursively. SREF and AREF placements build an affine transform
  from STRANS (reflection about the x axis), MAG and ANGLE, applied in the GDSII order:
  reflect, magnify, rotate, translate. AREF pitch vectors are `(xy[1] − xy[0]) / cols` and
  `(xy[2] − xy[0]) / rows`. Circular references and nesting deeper than 64 levels are
  errors.
- Coordinates are converted to µm with the library's database unit.
- Paths become polygons with mitered joins. Path types: 0 flush, 1 round (each cap
  approximated by a 16-segment half circle), 2 extended by half the width, 4 custom
  `BGNEXTN`/`ENDEXTN`. A negative width is absolute (not scaled by MAG). Each path keeps its
  width in µm, which is used for the resolution recommendation.
- BOUNDARY and BOX become polygons; TEXT becomes a label; NODE is ignored.

## 2. Simulation area and grid (`raster`)

The simulated area is the bounding box of all *Simulation area* layers if any are
assigned; otherwise the bounding box of conductor and terminal shapes plus a margin of 2
pixels on each side. Pixels are square:

    px = max(W, H) / resolution          (resolution ≥ 8)
    nx = ceil(W / px) (+ 2·margin),  ny = ceil(H / px) (+ 2·margin)

Grids above 60 million pixels are rejected. Image row 0 is at the top (`y0Um` is the top
edge), so `y` in image coordinates points down.

## 3. Exact-area rasterization (`raster`)

For each role (the conductor, and each terminal separately) all shapes are merged and
clipped to the grid rectangle with `i_overlay` (`OverlayRule::Intersect` against the grid
rectangle, `FillRule::NonZero` for the shapes), so overlapping shapes are counted once.
GDSII does not prescribe a polygon orientation, and mirrored placements reverse it, so every
contour is first given the same orientation; otherwise overlapping shapes with opposite
winding would cancel under the non-zero rule.

The merged contours are scan-converted with a signed-area accumulation rasterizer (the
technique used by font rasterizers): every edge adds the signed area it sweeps in each
pixel row to an accumulation buffer, and a prefix sum along each row yields the exact
covered fraction of every pixel, clamped to [0, 1]. The result is exact for straight edges,
including holes. Rows are processed in parallel bands of 32.

## 4. Pixel classification (`network`)

For every pixel:

- terminal coverage `t = min(Σ_k c_k, 1)` over all terminal maps; the pixel belongs to the
  terminal with the largest coverage if `t > 10⁻⁶`,
- conductor coverage `c = max(0, min(c_conductor, 1 − t))`.

A pixel with any terminal coverage is a *terminal pixel* and part of that terminal's
equipotential region. All other pixels with `c > 10⁻⁶` are *nodes*. If pixels of two
different terminals are 4-neighbours, the terminals short-circuit each other and the
simulation stops with an error.

## 5. Sub-pixel edge model (`network::pixel_geometry`)

A fully covered pixel (`c ≥ 0.9999`) conducts through all four sides with length 1. For a
partially covered pixel, CRAZE assumes a straight material edge inside the pixel:

1. The edge normal is estimated with a Sobel gradient of the 3×3 coverage neighbourhood.
   For conductor nodes the combined coverage (conductor + terminal) is used, so pixels next
   to a contact see the real outline. The normal angle is `atan2(g_y, g_x)` and points into
   the material.
2. The signed distance *d* of the edge line from the pixel center is found by bisection
   (48 steps) so that the part of the unit pixel on the material side has exactly the
   area *c*.
3. The conducting length ℓ_k of each of the four pixel sides is the part of that side on
   the material side of the line.

```text
  +-----------+         ℓ_up    = part of the top side inside the material
  |  .    ####|         ℓ_right = 1
  |    . #####|         ℓ_down  = 1
  |     ######|         ℓ_left  = part of the left side inside
  |   ########|
  +-----------+         area = c (exact from the rasterizer)
```

This turns slanted and curved edges, which a binary pixel mask would turn into staircases,
into a smooth boundary at the resolution of the coverage values.

## 6. Conductances (`network::Network::build`)

The network is a finite-volume discretization on the pixel grid: each node is a control
volume, and the current through each face is the conductance times the potential
difference. Conductances are in units of σ_s (lengths in pixels cancel, G = σ_s · w / l).

**Node to node.** The face width is the mean of the two conducting lengths seen from both
sides, and the center distance is one pixel:

    g_ij = ½ (ℓ_k(i) + ℓ_opp(k)(j))

**Node to a fully covered terminal pixel.** The terminal is equipotential up to the shared
face, which is half a pixel from the node center:

    g = ½ (ℓ_k + 1) / ½

**Node to a partially covered terminal pixel.** The contact interface is modeled as the
straight edge line inside the terminal pixel (angle θ_t, distance d_t from its sub-pixel
model). The face width is the node's conducting length projected onto the interface
direction, and the length is the distance from the node center to the interface line:

    g = ℓ_k · |cos(φ_k − (θ_t + π/2))| / max(|n_t · o − d_t|, 10⁻⁴)

where φ_k is the direction of the shared pixel side, n_t the interface normal and o the
node center in the terminal pixel's coordinates. This keeps the effective contact position
sub-pixel accurate.

## 7. Connectivity and terminals

A breadth-first search starts at the plus terminal and walks through nodes and terminals
(a terminal connects all nodes adjacent to any of its pixels). If it does not reach the
minus terminal, there is no conducting path and the simulation stops with an error. A
second search from the minus terminal adds parts that hang only on the minus side. Nodes
not reached are *isolated islands*: they are removed from the system, reported as a
warning with their share of the conductor, and shown hatched in the result view. Floating
terminals that were reached become additional unknowns; unreached ones have no potential.

## 8. Linear system (`network::assemble`)

For every node *j*, Kirchhoff's current law gives

    Σ_n g_jn (V_j − V_n) = 0

Neighbours on the plus and minus terminals move to the right-hand side (V = 1 and V = 0).
A floating terminal *f* contributes an unknown V_f and its own equation
Σ_j g_jf (V_f − V_j) = 0, i.e. zero net current. Every unknown is connected to a fixed
terminal, so the matrix is symmetric positive definite. It is stored in CSR with sorted
columns (about five non-zeros per row).

## 9. Solvers (`solver`)

**Direct.** Sparse Cholesky factorization with `faer` (lower triangle, faer's default
fill-reducing ordering). Exact up to rounding and robust; memory grows faster than linearly
because of fill-in.

**Iterative.** Conjugate gradients with an incomplete Cholesky IC(0) preconditioner (same
sparsity as the matrix, computed row by row). If IC(0) breaks down (non-positive pivot),
Jacobi preconditioning is used instead. The start vector is 0.5 everywhere. Iteration stops
when the relative residual ‖b − Ax‖ / ‖b‖ reaches the tolerance (default 10⁻¹⁰) or after
the maximum number of iterations (default 200 000); a final residual above 100 × tolerance
is reported as non-convergence. Sparse matrix-vector products and vector operations run in
parallel; the triangular solves of the preconditioner are sequential. Cancellation is
checked every iteration.

**Auto** uses the direct solver up to 8 000 000 unknowns and the iterative solver above.
After solving, the relative residual is computed for the statistics in both cases.

## 10. Post-processing (`simulate::compute_fields`)

- Flow from node *j* towards neighbour *k*: F_jk = g_jk (V_j − V_k).
- Total current: I_plus = sum of the flows from the plus terminal into its neighbours;
  I_minus = sum of the flows into the minus terminal. The number of squares is
  N = 1 / I_plus. The *current balance error* |I_plus − I_minus| / I_plus is a check of
  the solution (typically 10⁻⁹ or better with the direct solver).
- Sheet current density per node, from the average of the two opposite face flows:

      j_x = ½ (F_right − F_left),   j_y = ½ (F_down − F_up)

  in "current per pixel width" for ΔV = 1 and R□ = 1 (image coordinates, y down).
- Potential per node; terminal pixels get their terminal's potential; NaN elsewhere.

The frontend converts these to physical units for the chosen operating point (voltage V
across plus/minus, sheet resistance R□, pixel size px):

    K = |j| · V / (R□ · px)                 sheet current density, A/m
    J = K · σ_layer · R□                    current density in one layer of the stack, A/m²
    p = K² · R□                             dissipated power per area, W/m²

For a layer stack, J is reported for the most conductive layer (largest σ · R□), since
parallel layers see the same electric field. For a measured R□ the thickness is unknown and
only K is shown.

The **resolution study** in the app runs the resolutions r/4, r/2, r and 2r (the last only
if ≤ 4000) and applies Richardson extrapolation to the last three results N₁, N₂, N₃ with
refinement ratio q:

    order  p = ln((N₁ − N₂) / (N₂ − N₃)) / ln q
    N_∞    = N₃ − (N₂ − N₃) / (q^p − 1)
    error  ≈ |N₃ − N_∞| / N_∞

If the differences do not shrink monotonically, the last difference is reported as the
error estimate instead.

## 11. Validation

`crates/craze-core/tests/validation.rs` checks geometry against KLayout and physics against
analytic results. All physics cases use the direct solver and also require a current
balance error below 10⁻⁶.

| Test | Setup | Reference | CRAZE |
| --- | --- | --- | --- |
| `straight_bar_exact` | 100 × 10 µm bar, pads at both ends, 520 px | 10 squares | within 0.2 % |
| `rotated_bar` | same bar rotated by 30°, 400 and 800 px | 10 squares | 10.00003 |
| `corbino_ring` | annulus r₁ = 10 µm, r₂ = 40 µm, 300 and 600 px | ln(r₂/r₁) / 2π = 0.22064 | 0.22063, 0.22064 |
| `right_angle_corner` | L-shaped trace, width 10 µm, legs 60 µm, 700 px | 6 + 6 + 0.559 = 12.559 | 12.558 |
| `floating_terminal_bridges_gap` | bar with an ideal floating contact in a 10 µm gap | 9 squares | within 0.3 % |
| `iterative_matches_direct` | bar rotated by 17°, both solvers | direct result | within 10⁻⁷ |
| `hierarchy_matches_klayout` | hierarchical test cell (SREF/AREF, STRANS, MAG, ANGLE, path types), binary and text | merged layer areas from KLayout | within 0.2 % |
| `text_and_binary_examples_agree` | examples as `.gds2` text and binary `.gds` | each other | identical areas |
| `binary_roundtrip` | read → write → read | original records | identical |

The 0.559 squares of a square corner is the classical result for a 90° bend of uniform
width. The rotated bar shows the effect of the sub-pixel model: a binary pixel mask of the
same bar would have staircase edges and a resolution-dependent error of several percent.

Convergence on a real layout, the bundled Pt1000 meander (narrowest trace 20 µm), with
`craze sweep`:

| Resolution | Pixels across trace | Unknowns | Squares |
| --- | --- | --- | --- |
| 500 | 5.9 | 40 899 | 1005.343 |
| 1000 | 11.8 | 152 264 | 1005.582 |
| 2000 | 23.5 | 586 615 | 1005.673 |
| 4000 | 47 | 2 302 032 | 1005.745 |
| 6000 | 71 | 5 144 190 | 1005.732 |

Already at 6 pixels across the trace the result is within 0.04 % of the finest grid.

## 12. Limitations

- **Two-dimensional and uniform.** The film is a single sheet with one sheet resistance.
  All conductor layers are merged into that sheet; regions of different materials or
  thicknesses within one simulation are not supported. Vertical current flow (e.g. through
  vias or into the substrate) is not modeled.
- **Linear and isothermal.** Ohm's law with a temperature-dependent R□, but the whole
  structure is at one temperature. Self-heating and the resulting non-uniform resistivity
  are not coupled back.
- **Ideal contacts.** Terminals are perfect equipotentials without contact resistance. A
  pixel with any terminal coverage belongs to the terminal; the contact edge inside it is
  still modeled at sub-pixel accuracy via the conductance (section 6).
- **One edge per pixel.** The sub-pixel model assumes at most one straight edge per pixel.
  Features narrower than about two pixels (thin slits, narrow gaps between traces, sharp
  tips) are represented poorly. Hence the guideline of at least 10 pixels across the
  narrowest trace.
- **Corner singularities.** At re-entrant (inner) corners the exact current density is
  infinite. The total resistance converges, but peak current density and power density at
  such corners grow with the resolution.
- **Path geometry.** Joins are mitered without a miter limit, as in the GDSII definition;
  very acute angles produce long spikes. Round caps are polygons with 16 segments.
- **Size.** At most 60 million pixels. The direct solver's memory use grows with fill-in;
  the iterative solver is used automatically above 8 million unknowns.
- **Optimizer.** Only a uniform scale factor for all conductor path widths. Polygons,
  individual segments and centerlines are not changed. Widths are rounded to the database
  unit.
