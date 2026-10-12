// Typed wrappers around the Tauri commands.
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  Example,
  Fields,
  LayerKey,
  LayoutGeometry,
  LayoutInfo,
  Material,
  OptimizeResult,
  OptimizeStep,
  Progress,
  SimSettings,
  Setup,
  StudyPoint,
  Summary,
} from "./types";

export const openLayout = (path: string, topCell?: string) =>
  invoke<LayoutInfo>("open_layout", { path, topCell: topCell ?? null });

export async function layoutGeometry(): Promise<LayoutGeometry> {
  const buf = await invoke<ArrayBuffer>("layout_geometry");
  const dv = new DataView(buf);
  let o = 0;
  const cx = dv.getFloat64(o, true);
  const cy = dv.getFloat64(o + 8, true);
  o += 16;
  const nLayers = dv.getUint32(o, true);
  o += 4;
  const layers: LayoutGeometry["layers"] = [];
  for (let l = 0; l < nLayers; l++) {
    const layer = dv.getInt32(o, true);
    const datatype = dv.getInt32(o + 4, true);
    const nPoly = dv.getUint32(o + 8, true);
    o += 12;
    const polygons: Float32Array[] = [];
    for (let p = 0; p < nPoly; p++) {
      const n = dv.getUint32(o, true);
      o += 4;
      // Copy because the offset may not be 4-byte aligned relative to the buffer start.
      polygons.push(new Float32Array(buf.slice(o, o + n * 8)));
      o += n * 8;
    }
    layers.push({ key: { layer, datatype }, polygons });
  }
  return { cx, cy, layers };
}

export function runSimulation(setup: Setup, settings: SimSettings, onProgress: (p: Progress) => void) {
  const ch = new Channel<Progress>();
  ch.onmessage = onProgress;
  return invoke<Summary>("run_simulation", { setup, settings, onProgress: ch });
}

export async function resultFields(): Promise<Fields> {
  const buf = await invoke<ArrayBuffer>("result_fields");
  const dv = new DataView(buf);
  const nx = dv.getUint32(0, true);
  const ny = dv.getUint32(4, true);
  const n = nx * ny;
  const f = (i: number) => new Float32Array(buf, 8 + i * 4 * n, n);
  return {
    nx,
    ny,
    potential: f(0),
    jx: f(1),
    jy: f(2),
    coverage: f(3),
    material: new Uint8Array(buf, 8 + 16 * n, n),
  };
}

export const cancelJob = () => invoke<void>("cancel_job");

export function resolutionStudy(
  setup: Setup,
  settings: SimSettings,
  resolutions: number[],
  onPoint: (p: StudyPoint) => void,
) {
  const ch = new Channel<StudyPoint>();
  ch.onmessage = onPoint;
  return invoke<StudyPoint[]>("resolution_study", { setup, settings, resolutions, onPoint: ch });
}

export function runOptimization(
  args: {
    setup: Setup;
    settings: SimSettings;
    sheetResistance: number;
    targetOhm: number;
    tolerance: number;
    maxIterations: number;
  },
  onStep: (s: OptimizeStep) => void,
  onProgress: (p: Progress) => void,
) {
  const steps = new Channel<OptimizeStep>();
  steps.onmessage = onStep;
  const prog = new Channel<Progress>();
  prog.onmessage = onProgress;
  return invoke<OptimizeResult>("run_optimization", { ...args, onStep: steps, onProgress: prog });
}

export const saveOptimizedLayout = (path: string, scale: number, conductorKeys: LayerKey[]) =>
  invoke<void>("save_optimized_layout", { path, scale, conductorKeys });

export const materialLibrary = () => invoke<Material[]>("material_library");
export const writeTextFile = (path: string, contents: string) => invoke<void>("write_text_file", { path, contents });
export const readTextFile = (path: string) => invoke<string>("read_text_file", { path });
export const writeBase64File = (path: string, data: string) => invoke<void>("write_base64_file", { path, data });
export const loadPrefs = () => invoke<unknown>("load_prefs");
export const savePrefs = (prefs: unknown) => invoke<void>("save_prefs", { prefs });
export const listExamples = () => invoke<Example[]>("list_examples");
export const startupFile = () => invoke<string | null>("startup_file");
