// Central application state (Svelte 5 runes) and actions.
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { ColormapName } from "./colormaps";
import { setLocale } from "./format";
import type { DisplayField } from "./display";
import { sheetResistance } from "./physics";
import {
  keyStr,
  sameKey,
  type Example,
  type Fields,
  type LayerKey,
  type LayoutGeometry,
  type LayoutInfo,
  type Material,
  type OptimizeResult,
  type OptimizeStep,
  type Role,
  type SimSettings,
  type Setup,
  type Stack,
  type StudyPoint,
  type Summary,
} from "./types";

export type Step = "layout" | "layers" | "material" | "simulate" | "results" | "optimize";
export type FieldKind = "potential" | "current" | "power" | "material";
export type Theme = "system" | "light" | "dark";
export type Tool = "pan" | "cut";

export interface Toast {
  id: number;
  kind: "info" | "ok" | "warn" | "error";
  text: string;
}

export interface Recent {
  path: string;
  name: string;
  kind: "layout" | "project";
}

interface Preset {
  layers: Setup["layers"];
  terminals: Setup["terminals"];
  source: number;
  sink: number;
}

interface Prefs {
  lang: "de" | "en";
  theme: Theme;
  recent: Recent[];
  presets: Record<string, Preset>;
  colormap: ColormapName;
}

export interface ProjectFile {
  format: "craze-project";
  version: 1;
  layoutPath: string;
  topCell: string;
  setup: Setup;
  settings: SimSettings;
  stack: Stack;
  temperature: number;
  excitation: { mode: "voltage" | "current"; value: number };
  optimize: { target: number; tolerancePct: number; maxIterations: number };
  view: { field: FieldKind; colormap: ColormapName; log: boolean };
}

const DEFAULT_STACK: Stack = {
  mode: "layers",
  layers: [
    { material: { name: "Platin", resistivity: 1.06e-7, tcr: 3.92e-3 }, thicknessNm: 100 },
    { material: { name: "Tantal", resistivity: 1.31e-7, tcr: 3.5e-3 }, thicknessNm: 10 },
  ],
};

const DEFAULT_SETTINGS: SimSettings = { resolution: 1000, solver: "auto", tolerance: 1e-10, maxIterations: 200000 };

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));

class Store {
  // --- preferences ---
  lang = $state<"de" | "en">("de");
  theme = $state<Theme>("system");
  recent = $state<Recent[]>([]);
  presets = $state<Record<string, Preset>>({});
  materials = $state<Material[]>([]);
  examples = $state<Example[]>([]);

  // --- navigation / ui ---
  step = $state<Step>("layout");
  toasts = $state<Toast[]>([]);
  showHelp = $state(false);
  busyLoading = $state(false);

  // --- project data ---
  projectPath = $state<string | null>(null);
  savedSnapshot = $state<string>("");
  layout = $state<LayoutInfo | null>(null);
  geometry = $state.raw<LayoutGeometry | null>(null);
  setup = $state<Setup | null>(null);
  settings = $state<SimSettings>(clone(DEFAULT_SETTINGS));
  stack = $state<Stack>(clone(DEFAULT_STACK));
  temperature = $state(20);
  excitation = $state<{ mode: "voltage" | "current"; value: number }>({ mode: "voltage", value: 1 });
  hiddenLayers = $state<string[]>([]);

  // --- results ---
  summary = $state<Summary | null>(null);
  fields = $state.raw<Fields | null>(null);
  display = $state.raw<DisplayField | null>(null);
  simSignature = $state<string>("");
  study = $state<StudyPoint[]>([]);
  optSteps = $state<OptimizeStep[]>([]);
  optResult = $state<OptimizeResult | null>(null);
  optimize = $state({ target: 1000, tolerancePct: 0.2, maxIterations: 10 });

  // --- view ---
  field = $state<FieldKind>("current");
  colormap = $state<ColormapName>("inferno");
  logScale = $state(false);
  rangeMode = $state<"auto" | "full">("auto");
  showArrows = $state(false);
  showIsolines = $state(false);
  showOutlines = $state(false);
  viewMode = $state<"layout" | "result">("layout");
  tool = $state<Tool>("pan");
  cut = $state<{ a: [number, number]; b: [number, number] } | null>(null);
  focusRequest = $state<{ x: number; y: number; n: number } | null>(null);

  // --- running jobs ---
  job = $state<{ kind: "simulate" | "study" | "optimize"; message: string; fraction: number | null } | null>(null);
  lastError = $state<string | null>(null);

  // ---------------------------------------------------------------------------------
  // Derived
  // ---------------------------------------------------------------------------------

  sheetResistance = $derived(sheetResistance(this.stack, this.temperature));

  problems = $derived.by(() => {
    const s = this.setup;
    const l = this.layout;
    if (!s || !l) return [] as string[];
    const de = this.lang === "de";
    const out: string[] = [];
    if (!s.layers.some((a) => a.role.type === "conductor"))
      out.push(de ? "Es ist kein Layer als Leiter zugeordnet." : "No layer is assigned as conductor.");
    for (const [idx, label] of [
      [s.source, de ? "Plus" : "plus"],
      [s.sink, de ? "Minus" : "minus"],
    ] as const) {
      const has = s.layers.some((a) => a.role.type === "terminal" && a.role.id === idx);
      const name = s.terminals[idx]?.name ?? label;
      if (!has)
        out.push(
          de ? `Terminal „${name}“ (${label}) hat keinen Layer.` : `Terminal “${name}” (${label}) has no layer.`,
        );
    }
    if (s.source === s.sink) out.push(de ? "Plus und Minus müssen verschieden sein." : "Plus and minus must differ.");
    if (!(this.sheetResistance > 0 && isFinite(this.sheetResistance)))
      out.push(de ? "Der Flächenwiderstand ist ungültig." : "The sheet resistance is invalid.");
    return out;
  });

  /** Signature of everything that affects the geometric simulation result. */
  currentSignature = $derived(
    JSON.stringify({ p: this.layout?.path, c: this.layout?.topCell, s: this.setup, r: this.settings }),
  );
  resultStale = $derived(!!this.summary && this.simSignature !== this.currentSignature);

  snapshot = $derived(this.layout && this.setup ? JSON.stringify(this.projectData()) : "");
  dirty = $derived(!!this.layout && this.snapshot !== this.savedSnapshot);

  conductorKeys = $derived((this.setup?.layers ?? []).filter((a) => a.role.type === "conductor").map((a) => a.key));
  conductorHasPaths = $derived(
    (this.layout?.layers ?? []).some((l) => l.paths > 0 && this.conductorKeys.some((k) => sameKey(k, l.key))),
  );
  minConductorPathWidth = $derived.by(() => {
    let m = Infinity;
    for (const l of this.layout?.layers ?? [])
      if (l.minPathWidthUm && this.conductorKeys.some((k) => sameKey(k, l.key))) m = Math.min(m, l.minPathWidthUm);
    return isFinite(m) ? m : null;
  });

  // ---------------------------------------------------------------------------------
  // Init & preferences
  // ---------------------------------------------------------------------------------

  async init() {
    try {
      const p = (await api.loadPrefs()) as Partial<Prefs> | null;
      if (p) {
        if (p.lang) this.lang = p.lang;
        if (p.theme) this.theme = p.theme;
        if (p.recent) this.recent = p.recent;
        if (p.presets) this.presets = p.presets;
        if (p.colormap) this.colormap = p.colormap;
      }
    } catch {
      /* first start */
    }
    setLocale(this.lang);
    this.applyTheme();
    this.materials = await api.materialLibrary().catch(() => []);
    this.examples = await api.listExamples().catch(() => []);
    const startup = await api.startupFile().catch(() => null);
    if (startup) await this.openAny(startup);
  }

  persist() {
    const prefs: Prefs = {
      lang: this.lang,
      theme: this.theme,
      recent: $state.snapshot(this.recent),
      presets: $state.snapshot(this.presets),
      colormap: this.colormap,
    };
    api.savePrefs(prefs).catch(() => {});
  }

  setLang(l: "de" | "en") {
    this.lang = l;
    setLocale(l);
    this.persist();
  }

  setTheme(t: Theme) {
    this.theme = t;
    this.applyTheme();
    this.persist();
  }

  applyTheme() {
    if (this.theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", this.theme);
  }

  toast(kind: Toast["kind"], text: string, ms = 4500) {
    const id = Date.now() + Math.random();
    this.toasts.push({ id, kind, text });
    if (ms > 0) setTimeout(() => this.dismiss(id), ms);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  addRecent(r: Recent) {
    this.recent = [r, ...this.recent.filter((x) => x.path !== r.path)].slice(0, 8);
    this.persist();
  }

  // ---------------------------------------------------------------------------------
  // Layout
  // ---------------------------------------------------------------------------------

  async chooseFile() {
    const path = await openDialog({
      multiple: false,
      filters: [
        { name: "GDSII / CRAZE", extensions: ["gds", "gds2", "gdsii", "txt", "craze"] },
        { name: "GDSII", extensions: ["gds", "gds2", "gdsii", "txt"] },
        { name: "CRAZE-Projekt", extensions: ["craze"] },
      ],
    });
    if (typeof path === "string") await this.openAny(path);
  }

  async openAny(path: string) {
    if (path.toLowerCase().endsWith(".craze")) await this.openProject(path);
    else await this.openLayoutPath(path);
  }

  private resetResults() {
    this.summary = null;
    this.fields = null;
    this.simSignature = "";
    this.study = [];
    this.optSteps = [];
    this.optResult = null;
    this.cut = null;
    this.focusRequest = null;
    this.viewMode = "layout";
  }

  /** Opens a layout. With `keepSetup` the current setup is kept (project loading). */
  async openLayoutPath(path: string, topCell?: string, keepSetup = false): Promise<boolean> {
    this.busyLoading = true;
    try {
      const info = await api.openLayout(path, topCell);
      this.layout = info;
      this.geometry = await api.layoutGeometry();
      this.resetResults();
      this.hiddenLayers = [];
      if (!keepSetup) {
        this.projectPath = null;
        this.savedSnapshot = "";
        const sig = this.layerSignature();
        const preset = this.presets[sig];
        if (preset) {
          this.setup = clone(preset);
          this.toast("info", this.lang === "de" ? "Gespeicherte Layer-Zuordnung übernommen." : "Saved layer assignment applied.");
        } else {
          this.setup = clone(info.suggestion);
        }
        this.step = "layers";
        this.addRecent({ path, name: info.fileName, kind: "layout" });
      }
      // Sensible default resolution: narrowest conductor path ≥ 10 px.
      if (!keepSetup) this.settings.resolution = this.recommendedResolution() ?? 1000;
      return true;
    } catch (e) {
      this.toast("error", String(e), 8000);
      return false;
    } finally {
      this.busyLoading = false;
    }
  }

  async reloadLayout() {
    if (!this.layout) return;
    await this.openLayoutPath(this.layout.path, this.layout.topCell, true);
  }

  async selectTopCell(name: string) {
    if (!this.layout) return;
    const keep = clone(this.setup);
    await this.openLayoutPath(this.layout.path, name, true);
    this.setup = keep;
  }

  layerSignature(): string {
    return (this.layout?.layers ?? []).map((l) => keyStr(l.key)).join(",");
  }

  recommendedResolution(): number | null {
    const l = this.layout;
    const w = this.minConductorPathWidth;
    if (!l || !w) return null;
    const size = Math.max(l.bbox.max.x - l.bbox.min.x, l.bbox.max.y - l.bbox.min.y);
    const r = Math.ceil(((size / w) * 10) / 100) * 100;
    return Math.min(Math.max(r, 500), 6000);
  }

  roleOf(key: LayerKey): Role {
    return this.setup?.layers.find((a) => sameKey(a.key, key))?.role ?? { type: "ignore" };
  }

  setRole(key: LayerKey, role: Role) {
    if (!this.setup) return;
    const a = this.setup.layers.find((x) => sameKey(x.key, key));
    if (a) a.role = role;
    else this.setup.layers.push({ key, role });
  }

  addTerminal() {
    if (!this.setup) return;
    const n = this.setup.terminals.length + 1;
    this.setup.terminals.push({ name: `T${n}` });
  }

  removeTerminal(id: number) {
    const s = this.setup;
    if (!s || s.terminals.length <= 2) return;
    s.terminals.splice(id, 1);
    for (const a of s.layers) {
      if (a.role.type === "terminal") {
        if (a.role.id === id) a.role = { type: "ignore" };
        else if (a.role.id > id) a.role = { type: "terminal", id: a.role.id - 1 };
      }
    }
    if (s.source === id) s.source = 0;
    else if (s.source > id) s.source--;
    if (s.sink === id) s.sink = s.source === 0 ? 1 : 0;
    else if (s.sink > id) s.sink--;
  }

  savePreset() {
    if (!this.setup) return;
    this.presets[this.layerSignature()] = clone(this.setup);
    this.persist();
  }

  toggleLayer(key: LayerKey) {
    const k = keyStr(key);
    this.hiddenLayers = this.hiddenLayers.includes(k) ? this.hiddenLayers.filter((x) => x !== k) : [...this.hiddenLayers, k];
  }

  // ---------------------------------------------------------------------------------
  // Simulation
  // ---------------------------------------------------------------------------------

  async simulate() {
    if (!this.setup || this.job || this.problems.length) return;
    this.lastError = null;
    this.job = { kind: "simulate", message: "", fraction: null };
    const signature = this.currentSignature;
    try {
      const summary = await api.runSimulation($state.snapshot(this.setup), $state.snapshot(this.settings), (p) => {
        if (this.job) this.job = { kind: "simulate", message: p.message, fraction: p.fraction };
      });
      const fields = await api.resultFields();
      this.summary = summary;
      this.fields = fields;
      this.simSignature = signature;
      this.study = [];
      this.viewMode = "result";
      this.step = "results";
    } catch (e) {
      const msg = String(e);
      if (msg !== "Abgebrochen") {
        this.lastError = msg;
        this.toast("error", msg, 10000);
      }
    } finally {
      this.job = null;
    }
  }

  async cancel() {
    await api.cancelJob();
  }

  async runStudy() {
    if (!this.setup || this.job) return;
    const r = this.settings.resolution;
    // Constant refinement ratio 2 for Richardson extrapolation.
    const list = [Math.round(r / 4), Math.round(r / 2), r];
    if (r * 2 <= 4000) list.push(r * 2);
    this.study = [];
    this.job = { kind: "study", message: "", fraction: 0 };
    try {
      await api.resolutionStudy($state.snapshot(this.setup), $state.snapshot(this.settings), list, (p) => {
        this.study = [...this.study, p];
        this.job = { kind: "study", message: `${p.resolution} px`, fraction: this.study.length / list.length };
      });
    } catch (e) {
      if (String(e) !== "Abgebrochen") this.toast("error", String(e), 8000);
    } finally {
      this.job = null;
    }
  }

  async runOptimization() {
    if (!this.setup || this.job) return;
    this.optSteps = [];
    this.optResult = null;
    this.job = { kind: "optimize", message: "", fraction: null };
    try {
      const res = await api.runOptimization(
        {
          setup: $state.snapshot(this.setup),
          settings: $state.snapshot(this.settings),
          sheetResistance: this.sheetResistance,
          targetOhm: this.optimize.target,
          tolerance: this.optimize.tolerancePct / 100,
          maxIterations: this.optimize.maxIterations,
        },
        (s) => {
          this.optSteps = [...this.optSteps, s];
        },
        (p) => {
          if (this.job) this.job = { kind: "optimize", message: p.message, fraction: null };
        },
      );
      this.optResult = res;
      this.optSteps = res.steps;
    } catch (e) {
      if (String(e) !== "Abgebrochen") this.toast("error", String(e), 8000);
    } finally {
      this.job = null;
    }
  }

  async saveOptimized() {
    if (!this.optResult || !this.layout) return;
    const base = this.layout.fileName.replace(/\.(gds2?|gdsii|txt)$/i, "");
    const path = await saveDialog({
      defaultPath: `${base}_${this.lang === "de" ? "optimiert" : "optimized"}.gds`,
      filters: [{ name: "GDSII", extensions: ["gds"] }],
    });
    if (!path) return;
    try {
      await api.saveOptimizedLayout(path, this.optResult.bestScale, $state.snapshot(this.conductorKeys));
      this.toast("ok", this.lang === "de" ? "Optimiertes Layout gespeichert." : "Optimized layout saved.");
      return path;
    } catch (e) {
      this.toast("error", String(e));
    }
  }

  // ---------------------------------------------------------------------------------
  // Projects
  // ---------------------------------------------------------------------------------

  projectData(): ProjectFile | null {
    if (!this.layout || !this.setup) return null;
    return {
      format: "craze-project",
      version: 1,
      layoutPath: this.layout.path,
      topCell: this.layout.topCell,
      setup: this.setup,
      settings: this.settings,
      stack: this.stack,
      temperature: this.temperature,
      excitation: this.excitation,
      optimize: this.optimize,
      view: { field: this.field, colormap: this.colormap, log: this.logScale },
    };
  }

  async saveProject(saveAs = false) {
    const data = this.projectData();
    if (!data || !this.layout) return;
    let path = this.projectPath;
    if (!path || saveAs) {
      const base = this.layout.fileName.replace(/\.(gds2?|gdsii|txt)$/i, "");
      path = await saveDialog({ defaultPath: `${base}.craze`, filters: [{ name: "CRAZE", extensions: ["craze"] }] });
      if (!path) return;
    }
    try {
      const json = JSON.stringify(data, null, 2);
      await api.writeTextFile(path, json);
      this.projectPath = path;
      this.savedSnapshot = JSON.stringify(data);
      this.addRecent({ path, name: path.split(/[\\/]/).pop() ?? path, kind: "project" });
      this.toast("ok", this.lang === "de" ? "Projekt gespeichert." : "Project saved.");
    } catch (e) {
      this.toast("error", String(e));
    }
  }

  async openProject(path: string) {
    let data: ProjectFile;
    try {
      data = JSON.parse(await api.readTextFile(path));
      if (data.format !== "craze-project") throw new Error();
    } catch {
      this.toast("error", this.lang === "de" ? "Keine gültige CRAZE-Projektdatei." : "Not a valid CRAZE project file.");
      return;
    }
    let layoutPath = data.layoutPath;
    let ok = await this.openLayoutPath(layoutPath, data.topCell, true);
    if (!ok) {
      // Try next to the project file, then ask.
      const dir = path.replace(/[\\/][^\\/]*$/, "");
      const name = layoutPath.split(/[\\/]/).pop()!;
      const sep = path.includes("\\") ? "\\" : "/";
      layoutPath = `${dir}${sep}${name}`;
      ok = await this.openLayoutPath(layoutPath, data.topCell, true);
      if (!ok) {
        const picked = await openDialog({ multiple: false, title: name, filters: [{ name: "GDSII", extensions: ["gds", "gds2", "gdsii", "txt"] }] });
        if (typeof picked !== "string") return;
        layoutPath = picked;
        ok = await this.openLayoutPath(layoutPath, data.topCell, true);
        if (!ok) return;
      }
    }
    this.setup = data.setup;
    this.settings = { ...DEFAULT_SETTINGS, ...data.settings };
    this.stack = data.stack;
    this.temperature = data.temperature ?? 20;
    this.excitation = data.excitation ?? { mode: "voltage", value: 1 };
    this.optimize = data.optimize ?? this.optimize;
    if (data.view) {
      this.field = data.view.field;
      this.colormap = data.view.colormap;
      this.logScale = data.view.log;
    }
    this.projectPath = path;
    this.savedSnapshot = this.snapshot;
    this.step = "simulate";
    this.addRecent({ path, name: path.split(/[\\/]/).pop() ?? path, kind: "project" });
  }

  newProject() {
    if (this.dirty && !confirm(this.lang === "de" ? "Ungespeicherte Änderungen verwerfen?" : "Discard unsaved changes?")) return;
    this.layout = null;
    this.geometry = null;
    this.setup = null;
    this.projectPath = null;
    this.savedSnapshot = "";
    this.settings = clone(DEFAULT_SETTINGS);
    this.stack = clone(DEFAULT_STACK);
    this.temperature = 20;
    this.resetResults();
    this.step = "layout";
  }

  requestFocus(x: number, y: number) {
    this.focusRequest = { x, y, n: (this.focusRequest?.n ?? 0) + 1 };
  }
}

export const store = new Store();
