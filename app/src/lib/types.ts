// Types mirroring the Rust data structures (serde camelCase).

export interface LayerKey {
  layer: number;
  datatype: number;
}

export type Role =
  | { type: "ignore" }
  | { type: "conductor" }
  | { type: "terminal"; id: number }
  | { type: "region" };

export interface LayerAssignment {
  key: LayerKey;
  role: Role;
}

export interface TerminalDef {
  name: string;
}

export interface Setup {
  layers: LayerAssignment[];
  terminals: TerminalDef[];
  source: number;
  sink: number;
}

export type SolverKind = "auto" | "direct" | "iterative";

export interface SimSettings {
  resolution: number;
  solver: SolverKind;
  tolerance: number;
  maxIterations: number;
}

export interface Pt {
  x: number;
  y: number;
}

export interface BBox {
  min: Pt;
  max: Pt;
}

export interface LayerInfo {
  key: LayerKey;
  shapes: number;
  paths: number;
  areaUm2: number;
  bbox: BBox;
  minPathWidthUm: number | null;
}

export interface LabelInfo {
  key: LayerKey;
  text: string;
  x: number;
  y: number;
}

export interface LayoutInfo {
  path: string;
  fileName: string;
  dbUnitM: number;
  topCells: string[];
  topCell: string;
  layers: LayerInfo[];
  bbox: BBox;
  labels: LabelInfo[];
  suggestion: Setup;
}

export interface Grid {
  nx: number;
  ny: number;
  pxUm: number;
  x0Um: number;
  y0Um: number;
}

export interface Stats {
  conductorPixels: number;
  removedPixels: number;
  unknowns: number;
  nonzeros: number;
  solver: "direct" | "iterative";
  iterations: number;
  relativeResidual: number;
  currentBalanceError: number;
  timeRasterMs: number;
  timeBuildMs: number;
  timeSolveMs: number;
  timePostMs: number;
}

export interface Summary {
  grid: Grid;
  squares: number;
  terminalPotentials: number[];
  minPathWidthUm: number | null;
  suggestedResolution: number | null;
  stats: Stats;
  warnings: string[];
}

export interface Progress {
  stage: "rasterize" | "build" | "solve" | "postprocess";
  fraction: number | null;
  message: string;
}

export interface StudyPoint {
  resolution: number;
  squares: number;
  unknowns: number;
  timeMs: number;
}

export interface OptimizeStep {
  iteration: number;
  scale: number;
  resistance: number;
}

export interface OptimizeResult {
  steps: OptimizeStep[];
  bestScale: number;
  bestResistance: number;
  converged: boolean;
}

export interface Material {
  name: string;
  resistivity: number;
  tcr: number;
}

export interface StackLayer {
  material: Material;
  thicknessNm: number;
}

export type Stack =
  | { mode: "layers"; layers: StackLayer[] }
  | { mode: "measured"; sheetResistance: number; tcr: number; tRefC: number };

/** Result fields decoded from the binary buffer. */
export interface Fields {
  nx: number;
  ny: number;
  potential: Float32Array;
  jx: Float32Array;
  jy: Float32Array;
  coverage: Float32Array;
  material: Uint8Array;
}

/** Layout polygons for the preview, coordinates relative to (cx, cy). */
export interface LayoutGeometry {
  cx: number;
  cy: number;
  layers: { key: LayerKey; polygons: Float32Array[] }[];
}

export interface Example {
  name: string;
  path: string;
}

export const keyStr = (k: LayerKey) => `${k.layer}/${k.datatype}`;
export const sameKey = (a: LayerKey, b: LayerKey) => a.layer === b.layer && a.datatype === b.datatype;
