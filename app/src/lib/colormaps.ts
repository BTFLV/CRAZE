// Perceptual colormaps as 256-entry RGBA lookup tables.

export type ColormapName = "viridis" | "inferno" | "plasma" | "turbo" | "coolwarm" | "gray";

const STOPS: Record<ColormapName, string[]> = {
  viridis: ["#440154", "#482878", "#3e4989", "#31688e", "#26828e", "#1f9e89", "#35b779", "#6ece58", "#b5de2b", "#fde725"],
  inferno: ["#000004", "#1b0c41", "#4a0c6b", "#781c6d", "#a52c60", "#cf4446", "#ed6925", "#fb9b06", "#f7d13d", "#fcffa4"],
  plasma: ["#0d0887", "#46039f", "#7201a8", "#9c179e", "#bd3786", "#d8576b", "#ed7953", "#fb9f3a", "#fdca26", "#f0f921"],
  turbo: [
    "#30123b", "#4145ab", "#4675ed", "#39a2fc", "#1bcfd4", "#24eca6", "#61fc6c", "#a4fc3b",
    "#d1e834", "#f3c63a", "#fe9b2d", "#f36315", "#d93806", "#b11901", "#7a0403",
  ],
  coolwarm: ["#3b4cc0", "#6688ee", "#88bbff", "#b8d0f9", "#dddcdc", "#f5c4ad", "#f49a7b", "#de604d", "#b40426"],
  gray: ["#111111", "#f0f0f0"],
};

export const COLORMAPS: ColormapName[] = ["viridis", "inferno", "plasma", "turbo", "coolwarm", "gray"];

function hex(c: string): [number, number, number] {
  return [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16)];
}

const cache = new Map<ColormapName, Uint8Array>();

export function lut(name: ColormapName): Uint8Array {
  const hit = cache.get(name);
  if (hit) return hit;
  const stops = STOPS[name].map(hex);
  const out = new Uint8Array(256 * 4);
  for (let i = 0; i < 256; i++) {
    const t = (i / 255) * (stops.length - 1);
    const k = Math.min(Math.floor(t), stops.length - 2);
    const f = t - k;
    for (let c = 0; c < 3; c++) out[i * 4 + c] = Math.round(stops[k][c] * (1 - f) + stops[k + 1][c] * f);
    out[i * 4 + 3] = 255;
  }
  cache.set(name, out);
  return out;
}

export function cssGradient(name: ColormapName, direction = "to top"): string {
  const s = STOPS[name];
  return `linear-gradient(${direction}, ${s.map((c, i) => `${c} ${((i / (s.length - 1)) * 100).toFixed(1)}%`).join(", ")})`;
}

/** Colors for terminals (index = terminal id). Plus is red, minus is blue like in CRAZE 1. */
export const TERMINAL_COLORS = ["#e5484d", "#3e7bfa", "#30a46c", "#f5a524", "#9b5de5", "#00a2c7", "#e5539b", "#7c8b9f"];

/** Distinct colors for raw GDS layers in the layer manager (before roles are assigned). */
export function layerColor(i: number): string {
  const palette = ["#7c6cf0", "#2eb5a8", "#e0884a", "#d65a8b", "#5b8def", "#9bb83a", "#c9a227", "#8a6ad1", "#4fa3c7", "#cf6b5d"];
  return palette[i % palette.length];
}
