// Number formatting with SI prefixes and locale-aware decimal separators.

import { loc } from "./locale.svelte";

export function setLocale(lang: "de" | "en") {
  loc.value = lang === "de" ? "de-DE" : "en-US";
}

const PREFIXES: [number, string][] = [
  [1e12, "T"],
  [1e9, "G"],
  [1e6, "M"],
  [1e3, "k"],
  [1, ""],
  [1e-3, "m"],
  [1e-6, "µ"],
  [1e-9, "n"],
  [1e-12, "p"],
];

/** Thousands separators only from 10 000 on, so "3.920" is never mistaken for 3,92. */
const group = (v: number) => Math.abs(v) >= 1e4;

export function num(v: number, digits = 3, maxFrac?: number, grouping = true): string {
  if (!isFinite(v)) return "–";
  const abs = Math.abs(v);
  if (abs !== 0 && (abs < 1e-3 || abs >= 1e7)) {
    const [m, e] = v.toExponential(Math.max(0, digits - 1)).split("e");
    const mant = String(parseFloat(m));
    return `${loc.value.startsWith("de") ? mant.replace(".", ",") : mant}e${parseInt(e)}`;
  }
  const frac = maxFrac ?? (abs >= 100 ? Math.max(0, digits - 3) : abs >= 10 ? Math.max(0, digits - 2) : digits - 1);
  return new Intl.NumberFormat(loc.value, {
    maximumFractionDigits: Math.max(0, frac),
    minimumFractionDigits: 0,
    useGrouping: grouping && group(v),
  }).format(v);
}

/** Fixed number of significant digits, e.g. sig(986.238, 5) → "986,24". */
export function sig(v: number, digits = 4): string {
  if (!isFinite(v)) return "–";
  if (v === 0) return "0";
  return new Intl.NumberFormat(loc.value, { maximumSignificantDigits: digits, useGrouping: group(v) }).format(v);
}

export function si(v: number, unit: string, digits = 4): string {
  if (!isFinite(v)) return "–";
  if (v === 0) return `0 ${unit}`;
  const abs = Math.abs(v);
  for (const [f, p] of PREFIXES) {
    if (abs >= f * 0.99999) return `${sig(v / f, digits)} ${p}${unit}`;
  }
  return `${sig(v / 1e-12, digits)} p${unit}`;
}

export function sci(v: number, digits = 2): string {
  if (!isFinite(v)) return "–";
  const s = v.toExponential(digits);
  const [m, e] = s.split("e");
  const mant = loc.value.startsWith("de") ? m.replace(".", ",") : m;
  return `${mant}·10^${parseInt(e)}`;
}

export function length(um: number): string {
  const abs = Math.abs(um);
  if (abs >= 1000) return `${sig(um / 1000, 4)} mm`;
  if (abs >= 1) return `${sig(um, 4)} µm`;
  return `${sig(um * 1000, 4)} nm`;
}

export function duration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${sig(ms / 1000, 3)} s`;
}

export function parseNum(s: string): number {
  return parseFloat(s.replace(/\s/g, "").replace(",", "."));
}

/** Number for input fields: never grouped. */
export function inputNum(v: number, digits = 6): string {
  return num(v, digits, undefined, false);
}
