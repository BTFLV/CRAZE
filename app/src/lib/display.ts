// Converts normalized solver fields into displayable physical quantities.
import type { Fields, Summary } from "./types";

export type FieldKind = "potential" | "current" | "power" | "material";

export interface DisplayField {
  kind: FieldKind;
  /** Value per pixel in display units (NaN = not shown). */
  values: Float32Array;
  /** Potential in volts (for equipotential lines). */
  potentialV: Float32Array;
  min: number;
  max: number;
  unit: string;
  log: boolean;
  isoStep: number;
  /** Location and value of the maximum (hotspot). */
  peak: { value: number; x: number; y: number } | null;
}

export interface DisplayParams {
  kind: FieldKind;
  voltage: number;
  rs: number;
  /** Factor K → J of the most conductive layer (1/m), null if unknown. */
  volumeFactor: number | null;
  log: boolean;
  range: "auto" | "full";
}

/** Normalized |j| per pixel (current per pixel width for ΔV = 1, R□ = 1). */
export function jMagnitude(f: Fields): Float32Array {
  const n = f.nx * f.ny;
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) out[i] = Math.hypot(f.jx[i], f.jy[i]);
  return out;
}

function percentile(values: Float32Array, mask: (i: number) => boolean, p: number): number {
  const sample: number[] = [];
  const n = values.length;
  const stride = Math.max(1, Math.floor(n / 400000));
  for (let i = 0; i < n; i += stride) if (mask(i)) sample.push(values[i]);
  if (!sample.length) return 0;
  sample.sort((a, b) => a - b);
  return sample[Math.min(sample.length - 1, Math.floor(p * (sample.length - 1)))];
}

/** Sheet current density scale: normalized j → A/m. */
export function kScale(summary: Summary, voltage: number, rs: number): number {
  return voltage / rs / (summary.grid.pxUm * 1e-6);
}

export function buildDisplay(f: Fields, jmag: Float32Array, summary: Summary, p: DisplayParams): DisplayField {
  const n = f.nx * f.ny;
  const values = new Float32Array(n);
  const potentialV = new Float32Array(n);
  const ks = kScale(summary, p.voltage, p.rs);
  for (let i = 0; i < n; i++) potentialV[i] = f.potential[i] * p.voltage;

  let unit = "";
  let scale = 1;
  let squared = false;
  if (p.kind === "potential") {
    unit = "V";
    values.set(potentialV);
  } else if (p.kind === "current") {
    if (p.volumeFactor) {
      unit = "A/mm²";
      scale = (ks * p.volumeFactor) / 1e6;
    } else {
      unit = "A/m";
      scale = ks;
    }
  } else if (p.kind === "power") {
    unit = "W/mm²";
    scale = (ks * ks * p.rs) / 1e6;
    squared = true;
  }

  let peakIdx = -1;
  let peakVal = -Infinity;
  if (p.kind === "current" || p.kind === "power" || p.kind === "material") {
    for (let i = 0; i < n; i++) {
      if (f.material[i] !== 1) {
        values[i] = NaN;
        continue;
      }
      const j = jmag[i];
      const v = squared ? j * j * scale : j * scale;
      values[i] = p.kind === "material" ? 0 : v;
      if (v > peakVal) {
        peakVal = v;
        peakIdx = i;
      }
    }
  }

  let min = 0;
  let max = 1;
  const isCond = (i: number) => f.material[i] === 1 && values[i] > 0;
  if (p.kind === "potential") {
    min = 0;
    let vmax = p.voltage;
    for (const v of summary.terminalPotentials) vmax = Math.max(vmax, v * p.voltage);
    max = vmax || 1;
  } else if (p.kind !== "material") {
    const hi = p.range === "auto" ? percentile(values, isCond, 0.995) : peakVal;
    max = hi > 0 ? hi : 1;
    if (p.log) {
      const lo = p.range === "auto" ? percentile(values, isCond, 0.01) : Math.max(max * 1e-6, 1e-30);
      min = Math.max(lo, max * 1e-4);
    } else {
      min = 0;
    }
  }

  let peak: DisplayField["peak"] = null;
  if (peakIdx >= 0 && p.kind !== "potential") {
    const g = summary.grid;
    const ix = peakIdx % f.nx;
    const iy = Math.floor(peakIdx / f.nx);
    peak = { value: peakVal, x: g.x0Um + (ix + 0.5) * g.pxUm, y: g.y0Um - (iy + 0.5) * g.pxUm };
  }

  return {
    kind: p.kind,
    values,
    potentialV,
    min,
    max,
    unit,
    log: p.log && p.kind !== "potential" && p.kind !== "material",
    isoStep: Math.max(p.voltage, 1e-30) / 20,
    peak,
  };
}

/** Samples a field along a line (world coordinates, µm). */
export function sampleLine(
  values: Float32Array,
  summary: Summary,
  a: [number, number],
  b: [number, number],
): { d: number[]; v: (number | null)[]; ixs: number[] } {
  const g = summary.grid;
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const steps = Math.max(2, Math.min(5000, Math.ceil((len / g.pxUm) * 2)));
  const d: number[] = [];
  const v: (number | null)[] = [];
  const ixs: number[] = [];
  for (let s = 0; s <= steps; s++) {
    const t = s / steps;
    const x = a[0] + (b[0] - a[0]) * t;
    const y = a[1] + (b[1] - a[1]) * t;
    const ix = Math.floor((x - g.x0Um) / g.pxUm);
    const iy = Math.floor((g.y0Um - y) / g.pxUm);
    d.push(len * t);
    if (ix < 0 || iy < 0 || ix >= g.nx || iy >= g.ny) {
      v.push(null);
      ixs.push(-1);
      continue;
    }
    const i = iy * g.nx + ix;
    const val = values[i];
    v.push(isNaN(val) ? null : val);
    ixs.push(i);
  }
  return { d, v, ixs };
}

/** Current (A) crossing the line a→b, computed from the sheet current density vector. */
export function lineCurrent(f: Fields, summary: Summary, a: [number, number], b: [number, number], voltage: number, rs: number): number {
  const g = summary.grid;
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
  if (len <= 0) return 0;
  // Unit normal in image coordinates (x right, y down).
  const tx = (b[0] - a[0]) / len;
  const ty = -(b[1] - a[1]) / len;
  const nx = -ty;
  const ny = tx;
  const steps = Math.max(2, Math.ceil((len / g.pxUm) * 4));
  const ds = (len / steps) * 1e-6;
  const ks = kScale(summary, voltage, rs);
  let sum = 0;
  for (let s = 0; s < steps; s++) {
    const t = (s + 0.5) / steps;
    const x = a[0] + (b[0] - a[0]) * t;
    const y = a[1] + (b[1] - a[1]) * t;
    const ix = Math.floor((x - g.x0Um) / g.pxUm);
    const iy = Math.floor((g.y0Um - y) / g.pxUm);
    if (ix < 0 || iy < 0 || ix >= g.nx || iy >= g.ny) continue;
    const i = iy * g.nx + ix;
    if (f.material[i] !== 1) continue;
    sum += (f.jx[i] * nx + f.jy[i] * ny) * ks * ds;
  }
  return sum;
}
