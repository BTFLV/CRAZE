// Material/stack physics mirrored from craze-core for instant UI updates.
import type { Stack } from "./types";

export const T_REF_C = 20;

export function sheetResistance(stack: Stack, tC: number): number {
  if (stack.mode === "measured") return stack.sheetResistance * (1 + stack.tcr * (tC - stack.tRefC));
  let g = 0;
  for (const l of stack.layers) {
    if (l.thicknessNm > 0 && l.material.resistivity > 0) {
      const rho = l.material.resistivity * (1 + l.material.tcr * (tC - T_REF_C));
      g += (l.thicknessNm * 1e-9) / rho;
    }
  }
  return g > 0 ? 1 / g : Infinity;
}

/** Effective temperature coefficient of the stack around tC (1/K). */
export function effectiveTcr(stack: Stack, tC: number): number {
  const r0 = sheetResistance(stack, tC);
  const r1 = sheetResistance(stack, tC + 1);
  return (r1 - r0) / r0;
}

/**
 * Factor converting sheet current density K (A/m) into the volume current density of the
 * most conductive layer (A/m²). Null if layer thicknesses are unknown.
 */
export function volumeCurrentFactor(stack: Stack, tC: number): { factor: number; layer: string } | null {
  if (stack.mode === "measured") return null;
  const rs = sheetResistance(stack, tC);
  let best: { factor: number; layer: string } | null = null;
  for (const l of stack.layers) {
    if (!(l.thicknessNm > 0 && l.material.resistivity > 0)) continue;
    const sigma = 1 / (l.material.resistivity * (1 + l.material.tcr * (tC - T_REF_C)));
    const f = sigma * rs;
    if (!best || f > best.factor) best = { factor: f, layer: l.material.name };
  }
  return best;
}

export interface Electrical {
  resistance: number;
  voltage: number;
  current: number;
  power: number;
}

export function electrical(squares: number, rs: number, mode: "voltage" | "current", value: number): Electrical {
  const resistance = squares * rs;
  const voltage = mode === "voltage" ? value : value * resistance;
  const current = mode === "current" ? value : value / resistance;
  return { resistance, voltage, current, power: voltage * current };
}
