<script lang="ts">
  import { onMount } from "svelte";
  import { Hand, Maximize, Ruler, Layers3, Flame } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
  import { FieldRenderer } from "../lib/gl";
  import { TERMINAL_COLORS, cssGradient, layerColor, lut } from "../lib/colormaps";
  import { keyStr, type LayerKey } from "../lib/types";
  import { si, length, sig } from "../lib/format";
  import { kScale } from "../lib/display";
  import { t } from "../lib/i18n";
  import { viewerApi } from "../lib/viewer";

  let container: HTMLDivElement;
  let glCanvas: HTMLCanvasElement;
  let ovCanvas: HTMLCanvasElement;
  let renderer: FieldRenderer | null = null;
  let glError = $state<string | null>(null);

  let W = $state(800);
  let H = $state(600);
  let dpr = 1;
  const cam = { cx: 0, cy: 0, scale: 1 };
  let fittedFor = "";

  let hover = $state<{ sx: number; sy: number; x: number; y: number; lines: [string, string][] } | null>(null);
  let dragging: null | { mode: "pan" | "cut"; lastX: number; lastY: number } = null;

  const showResult = $derived(store.viewMode === "result" && !!store.fields && !!store.summary);

  // ---------------------------------------------------------------------------------
  // Coordinate helpers
  // ---------------------------------------------------------------------------------
  const toScreen = (x: number, y: number): [number, number] => [
    (x - cam.cx) * cam.scale + W / 2,
    H / 2 - (y - cam.cy) * cam.scale,
  ];
  const toWorld = (sx: number, sy: number): [number, number] => [
    (sx - W / 2) / cam.scale + cam.cx,
    (H / 2 - sy) / cam.scale + cam.cy,
  ];

  function bounds(): { x0: number; y0: number; x1: number; y1: number } | null {
    if (showResult && store.summary) {
      const g = store.summary.grid;
      return { x0: g.x0Um, y0: g.y0Um - g.ny * g.pxUm, x1: g.x0Um + g.nx * g.pxUm, y1: g.y0Um };
    }
    const b = store.layout?.bbox;
    return b ? { x0: b.min.x, y0: b.min.y, x1: b.max.x, y1: b.max.y } : null;
  }

  function fit() {
    const b = bounds();
    if (!b) return;
    const bw = Math.max(b.x1 - b.x0, 1e-6);
    const bh = Math.max(b.y1 - b.y0, 1e-6);
    cam.cx = (b.x0 + b.x1) / 2;
    cam.cy = (b.y0 + b.y1) / 2;
    cam.scale = Math.min((W - 80) / bw, (H - 80) / bh);
    redraw();
  }

  // ---------------------------------------------------------------------------------
  // Drawing
  // ---------------------------------------------------------------------------------
  let raf = 0;
  function redraw() {
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      draw();
    });
  }

  let pathCache: { geom: unknown; paths: Map<string, Path2D> } = { geom: null, paths: new Map() };
  function layerPaths(): Map<string, Path2D> {
    const g = store.geometry;
    if (pathCache.geom !== g) {
      const paths = new Map<string, Path2D>();
      for (const l of g?.layers ?? []) {
        const p = new Path2D();
        for (const poly of l.polygons) {
          if (poly.length < 6) continue;
          p.moveTo(poly[0], poly[1]);
          for (let i = 2; i < poly.length; i += 2) p.lineTo(poly[i], poly[i + 1]);
          p.closePath();
        }
        paths.set(keyStr(l.key), p);
      }
      pathCache = { geom: g, paths };
    }
    return pathCache.paths;
  }

  function cssVar(name: string) {
    return getComputedStyle(container).getPropertyValue(name).trim();
  }

  function layerStyle(key: LayerKey, idx: number): { fill: string | null; stroke: string; alpha: number; dash: boolean } {
    const role = store.roleOf(key);
    switch (role.type) {
      case "conductor":
        return { fill: "#8f9bb3", stroke: "#5d6a85", alpha: 0.9, dash: false };
      case "terminal":
        return { fill: TERMINAL_COLORS[role.id % 8], stroke: TERMINAL_COLORS[role.id % 8], alpha: 0.85, dash: false };
      case "region":
        return { fill: null, stroke: cssVar("--accent"), alpha: 1, dash: true };
      default:
        return { fill: layerColor(idx), stroke: layerColor(idx), alpha: 0.22, dash: false };
    }
  }

  function drawLayout(ctx: CanvasRenderingContext2D, outlinesOnly: boolean) {
    const g = store.geometry;
    const layout = store.layout;
    if (!g || !layout) return;
    const paths = layerPaths();
    const e = W / 2 + (g.cx - cam.cx) * cam.scale;
    const f = H / 2 - (g.cy - cam.cy) * cam.scale;
    ctx.save();
    ctx.setTransform(dpr * cam.scale, 0, 0, -dpr * cam.scale, dpr * e, dpr * f);
    const order = (k: LayerKey) => {
      const r = store.roleOf(k).type;
      return r === "region" ? 0 : r === "ignore" ? 1 : r === "conductor" ? 2 : 3;
    };
    const layers = layout.layers.map((l, i) => ({ key: l.key, i })).sort((a, b) => order(a.key) - order(b.key));
    for (const { key, i } of layers) {
      if (store.hiddenLayers.includes(keyStr(key))) continue;
      const p = paths.get(keyStr(key));
      if (!p) continue;
      const st = layerStyle(key, i);
      if (outlinesOnly) {
        if (store.roleOf(key).type === "ignore") continue;
        ctx.globalAlpha = 0.9;
        ctx.strokeStyle = "#ffffff";
        ctx.lineWidth = 1 / cam.scale;
        ctx.setLineDash([]);
        ctx.stroke(p);
        continue;
      }
      if (st.fill) {
        ctx.globalAlpha = st.alpha;
        ctx.fillStyle = st.fill;
        ctx.fill(p, "nonzero");
      }
      ctx.globalAlpha = st.dash ? 1 : Math.min(1, st.alpha + 0.1);
      ctx.strokeStyle = st.stroke;
      ctx.lineWidth = (st.dash ? 1.5 : 1) / cam.scale;
      ctx.setLineDash(st.dash ? [6 / cam.scale, 4 / cam.scale] : []);
      ctx.stroke(p);
    }
    ctx.restore();
  }

  function niceLength(target: number): number {
    const p = Math.pow(10, Math.floor(Math.log10(target)));
    for (const m of [1, 2, 5, 10]) if (m * p >= target) return m * p;
    return 10 * p;
  }

  function drawScaleBar(ctx: CanvasRenderingContext2D) {
    const um = niceLength(110 / cam.scale);
    const px = um * cam.scale;
    const x = 16;
    const y = H - 18;
    ctx.save();
    ctx.scale(dpr, dpr);
    const fg = cssVar("--text");
    ctx.strokeStyle = fg;
    ctx.fillStyle = fg;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(x, y - 5);
    ctx.lineTo(x, y);
    ctx.lineTo(x + px, y);
    ctx.lineTo(x + px, y - 5);
    ctx.stroke();
    ctx.font = `500 11px ${cssVar("--font")}`;
    ctx.fillText(length(um), x + 4, y - 7);
    ctx.restore();
  }

  function drawArrows(ctx: CanvasRenderingContext2D) {
    const f = store.fields;
    const s = store.summary;
    const d = store.display;
    if (!f || !s || !d) return;
    const g = s.grid;
    const spacing = 26;
    // Reference magnitude: 99.5 % of |j| (normalized units) taken from the display range.
    let ref = 0;
    const n = f.nx * f.ny;
    const stride = Math.max(1, Math.floor(n / 200000));
    const mags: number[] = [];
    for (let i = 0; i < n; i += stride) if (f.material[i] === 1) mags.push(Math.hypot(f.jx[i], f.jy[i]));
    mags.sort((a, b) => a - b);
    ref = mags[Math.floor(mags.length * 0.98)] || 1;
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.lineCap = "round";
    // One arrow per screen cell, placed on the conductor sample closest to the cell center,
    // so narrow traces are not skipped when zoomed out.
    const sub = 7;
    for (let cy0 = 0; cy0 < H; cy0 += spacing) {
      for (let cx0 = 0; cx0 < W; cx0 += spacing) {
        let best = -1;
        let bestD = Infinity;
        let bx = 0;
        let by = 0;
        for (let a = 0; a < sub; a++) {
          for (let b = 0; b < sub; b++) {
            const sx = cx0 + ((a + 0.5) / sub) * spacing;
            const sy = cy0 + ((b + 0.5) / sub) * spacing;
            const [x, y] = toWorld(sx, sy);
            const ix = Math.floor((x - g.x0Um) / g.pxUm);
            const iy = Math.floor((g.y0Um - y) / g.pxUm);
            if (ix < 0 || iy < 0 || ix >= g.nx || iy >= g.ny) continue;
            const i = iy * g.nx + ix;
            if (f.material[i] !== 1) continue;
            const dd = (sx - cx0 - spacing / 2) ** 2 + (sy - cy0 - spacing / 2) ** 2;
            if (dd < bestD) {
              bestD = dd;
              best = i;
              bx = sx;
              by = sy;
            }
          }
        }
        if (best < 0) continue;
        const jx = f.jx[best];
        const jy = f.jy[best];
        const m = Math.hypot(jx, jy);
        if (m < ref * 0.02) continue;
        // Short arrows for narrow traces at low zoom: limit length to the cell.
        const len = spacing * 0.8 * Math.min(1, Math.sqrt(m / ref));
        const ux = jx / m;
        const uy = jy / m;
        const x0 = bx - (ux * len) / 2;
        const y0 = by - (uy * len) / 2;
        const x1 = bx + (ux * len) / 2;
        const y1 = by + (uy * len) / 2;
        const head = Math.min(6, len * 0.45);
        for (const [w, c] of [
          [3.2, "rgba(0,0,0,0.55)"],
          [1.4, "#ffffff"],
        ] as const) {
          ctx.strokeStyle = c;
          ctx.lineWidth = w;
          ctx.beginPath();
          ctx.moveTo(x0, y0);
          ctx.lineTo(x1, y1);
          ctx.moveTo(x1 - ux * head - uy * head * 0.6, y1 - uy * head + ux * head * 0.6);
          ctx.lineTo(x1, y1);
          ctx.lineTo(x1 - ux * head + uy * head * 0.6, y1 - uy * head - ux * head * 0.6);
          ctx.stroke();
        }
      }
    }
    ctx.restore();
  }

  function drawCut(ctx: CanvasRenderingContext2D) {
    const c = store.cut;
    if (!c) return;
    const [ax, ay] = toScreen(c.a[0], c.a[1]);
    const [bx, by] = toScreen(c.b[0], c.b[1]);
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.strokeStyle = "rgba(0,0,0,0.6)";
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.moveTo(ax, ay);
    ctx.lineTo(bx, by);
    ctx.stroke();
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2;
    ctx.setLineDash([6, 4]);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.font = `700 11px ${cssVar("--font")}`;
    for (const [x, y, l] of [
      [ax, ay, "A"],
      [bx, by, "B"],
    ] as const) {
      ctx.fillStyle = cssVar("--accent");
      ctx.beginPath();
      ctx.arc(x, y, 8, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#fff";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(l, x, y + 0.5);
    }
    ctx.restore();
  }

  function drawPeak(ctx: CanvasRenderingContext2D) {
    const d = store.display;
    if (!d?.peak || (d.kind !== "current" && d.kind !== "power")) return;
    const [x, y] = toScreen(d.peak.x, d.peak.y);
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(x, y, 9, 0, Math.PI * 2);
    ctx.stroke();
    ctx.strokeStyle = "rgba(0,0,0,0.6)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(x, y, 11, 0, Math.PI * 2);
    ctx.stroke();
    ctx.font = `600 11px ${cssVar("--font")}`;
    ctx.fillStyle = "#fff";
    ctx.strokeStyle = "rgba(0,0,0,0.7)";
    ctx.lineWidth = 3;
    ctx.lineJoin = "round";
    ctx.strokeText("max", x + 13, y - 8);
    ctx.fillText("max", x + 13, y - 8);
    ctx.restore();
  }

  function draw() {
    if (!ovCanvas) return;
    const devW = Math.round(W * dpr);
    const devH = Math.round(H * dpr);
    for (const c of [glCanvas, ovCanvas]) {
      if (c.width !== devW || c.height !== devH) {
        c.width = devW;
        c.height = devH;
      }
    }
    const ctx = ovCanvas.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, devW, devH);

    if (showResult && renderer && store.summary && store.display) {
      const g = store.summary.grid;
      const d = store.display;
      renderer.draw(devW, devH, {
        offX: g.x0Um - cam.cx,
        offY: g.y0Um - cam.cy,
        sizeX: g.nx * g.pxUm,
        sizeY: g.ny * g.pxUm,
        scale: cam.scale,
        viewW: W,
        viewH: H,
        min: d.min,
        max: d.max,
        log: d.log,
        iso: store.showIsolines,
        isoStep: d.isoStep,
        geometryOnly: d.kind === "material",
        condColor: "#8f9bb3",
      });
      if (store.showOutlines) drawLayout(ctx, true);
      if (store.showArrows) drawArrows(ctx);
      drawPeak(ctx);
      drawCut(ctx);
    } else {
      renderer?.clear(devW, devH);
      drawLayout(ctx, false);
    }
    drawScaleBar(ctx);
  }

  // ---------------------------------------------------------------------------------
  // Hover inspector
  // ---------------------------------------------------------------------------------
  function updateHover(sx: number, sy: number) {
    const [x, y] = toWorld(sx, sy);
    const lines: [string, string][] = [];
    const s = store.summary;
    const f = store.fields;
    if (showResult && s && f && store.display) {
      const g = s.grid;
      const ix = Math.floor((x - g.x0Um) / g.pxUm);
      const iy = Math.floor((g.y0Um - y) / g.pxUm);
      if (ix >= 0 && iy >= 0 && ix < g.nx && iy < g.ny) {
        const i = iy * g.nx + ix;
        const m = f.material[i];
        const U = currentVoltage();
        if (m >= 10) {
          const tid = m - 10;
          lines.push([t("layers.terminal"), store.setup?.terminals[tid]?.name ?? `T${tid + 1}`]);
          lines.push([t("results.potential"), si(f.potential[i] * U, "V")]);
        } else if (m === 1) {
          const ks = kScale(s, U, store.sheetResistance);
          const K = Math.hypot(f.jx[i], f.jy[i]) * ks;
          lines.push([t("results.potential"), si(f.potential[i] * U, "V")]);
          const vf = store.display.kind === "current" && store.display.unit === "A/mm²";
          lines.push([t("results.fieldCurrent"), vf ? `${sig(store.display.values[i], 4)} A/mm²` : si(K, "A/m")]);
          lines.push([t("results.fieldPower"), si((K * K * store.sheetResistance) / 1e6, "W/mm²")]);
          lines.push(["Bedeckung", `${Math.round(f.coverage[i] * 100)} %`]);
        } else if (m === 2) {
          lines.push(["", t("viewer.isolated")]);
        }
      }
    }
    hover = { sx, sy, x, y, lines };
  }

  function currentVoltage(): number {
    const s = store.summary;
    if (!s) return 1;
    if (store.excitation.mode === "voltage") return store.excitation.value;
    return store.excitation.value * s.squares * store.sheetResistance;
  }

  // ---------------------------------------------------------------------------------
  // Interaction
  // ---------------------------------------------------------------------------------
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const r = ovCanvas.getBoundingClientRect();
    const sx = e.clientX - r.left;
    const sy = e.clientY - r.top;
    const [wx, wy] = toWorld(sx, sy);
    const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015));
    cam.scale = Math.min(Math.max(cam.scale * factor, 1e-6), 1e6);
    cam.cx = wx - (sx - W / 2) / cam.scale;
    cam.cy = wy - (H / 2 - sy) / cam.scale;
    redraw();
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 && e.button !== 1) return;
    ovCanvas.setPointerCapture(e.pointerId);
    const r = ovCanvas.getBoundingClientRect();
    const sx = e.clientX - r.left;
    const sy = e.clientY - r.top;
    if (store.tool === "cut" && showResult && e.button === 0) {
      const w = toWorld(sx, sy);
      store.cut = { a: w, b: w };
      dragging = { mode: "cut", lastX: sx, lastY: sy };
    } else {
      dragging = { mode: "pan", lastX: sx, lastY: sy };
    }
  }

  function onPointerMove(e: PointerEvent) {
    const r = ovCanvas.getBoundingClientRect();
    const sx = e.clientX - r.left;
    const sy = e.clientY - r.top;
    if (dragging?.mode === "pan") {
      cam.cx -= (sx - dragging.lastX) / cam.scale;
      cam.cy += (sy - dragging.lastY) / cam.scale;
      dragging.lastX = sx;
      dragging.lastY = sy;
      redraw();
    } else if (dragging?.mode === "cut" && store.cut) {
      store.cut = { a: store.cut.a, b: toWorld(sx, sy) };
    }
    updateHover(sx, sy);
  }

  function onPointerUp(e: PointerEvent) {
    if (dragging?.mode === "cut" && store.cut) {
      const c = store.cut;
      if (Math.hypot(c.a[0] - c.b[0], c.a[1] - c.b[1]) * cam.scale < 4) store.cut = null;
    }
    dragging = null;
    ovCanvas.releasePointerCapture(e.pointerId);
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
    if (e.key === "f" || e.key === "F") fit();
    if (e.key === "Escape") store.tool = "pan";
  }

  // ---------------------------------------------------------------------------------
  // Effects
  // ---------------------------------------------------------------------------------
  onMount(() => {
    dpr = window.devicePixelRatio || 1;
    try {
      renderer = new FieldRenderer(glCanvas);
    } catch (e) {
      glError = String(e);
    }
    const ro = new ResizeObserver(() => {
      const r = container.getBoundingClientRect();
      W = Math.max(1, r.width);
      H = Math.max(1, r.height);
      dpr = window.devicePixelRatio || 1;
      redraw();
    });
    ro.observe(container);
    window.addEventListener("keydown", onKey);
    viewerApi.snapshot = snapshot;
    viewerApi.fit = fit;
    return () => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
    };
  });

  // Upload geometry textures for new results.
  $effect(() => {
    const f = store.fields;
    if (!renderer || !f) return;
    if (f.nx > renderer.maxTexture || f.ny > renderer.maxTexture) {
      glError = `Raster zu groß für die Grafikkarte (max. ${renderer.maxTexture} Pixel).`;
      return;
    }
    renderer.setGeometry(f.nx, f.ny, f.material, f.coverage);
  });

  $effect(() => {
    const d = store.display;
    if (renderer && d && store.fields) renderer.setValues(d.values, d.potentialV);
    redraw();
  });

  $effect(() => {
    renderer?.setColormap(store.colormap);
    redraw();
  });

  // Fit when a new layout or a first result arrives.
  $effect(() => {
    const key = `${store.layout?.path}|${store.layout?.topCell}|${showResult ? store.summary?.grid.nx : "l"}`;
    if (key !== fittedFor && W > 1) {
      const first = !fittedFor.startsWith(`${store.layout?.path}|${store.layout?.topCell}`);
      fittedFor = key;
      if (first) fit();
    }
  });

  // Redraw on any visual setting change.
  $effect(() => {
    void [
      store.showArrows,
      store.showIsolines,
      store.showOutlines,
      store.cut,
      store.viewMode,
      store.setup && JSON.stringify(store.setup.layers),
      store.hiddenLayers.length,
      store.geometry,
      store.theme,
      W,
      H,
    ];
    redraw();
  });

  $effect(() => {
    const r = store.focusRequest;
    if (!r) return;
    const target = Math.max(cam.scale, W / (80 * (store.summary?.grid.pxUm ?? 1)));
    const start = { ...cam };
    const t0 = performance.now();
    const anim = (now: number) => {
      const k = Math.min(1, (now - t0) / 350);
      const e = 1 - Math.pow(1 - k, 3);
      cam.cx = start.cx + (r.x - start.cx) * e;
      cam.cy = start.cy + (r.y - start.cy) * e;
      cam.scale = start.scale * Math.pow(target / start.scale, e);
      draw();
      if (k < 1) requestAnimationFrame(anim);
    };
    requestAnimationFrame(anim);
  });

  // ---------------------------------------------------------------------------------
  // Snapshot for export (field + overlay + legend)
  // ---------------------------------------------------------------------------------
  function snapshot(): string {
    draw();
    const c = document.createElement("canvas");
    c.width = glCanvas.width;
    c.height = glCanvas.height;
    const ctx = c.getContext("2d")!;
    ctx.fillStyle = cssVar("--viewer-bg");
    ctx.fillRect(0, 0, c.width, c.height);
    ctx.drawImage(glCanvas, 0, 0);
    ctx.drawImage(ovCanvas, 0, 0);
    const d = store.display;
    if (showResult && d && d.kind !== "material") {
      ctx.save();
      ctx.scale(dpr, dpr);
      const x = W - 70;
      const y = 50;
      const h = Math.min(260, H - 120);
      const grad = ctx.createLinearGradient(0, y + h, 0, y);
      const stops = 12;
      for (let i = 0; i <= stops; i++) {
        const tt = i / stops;
        grad.addColorStop(tt, legendColor(tt));
      }
      ctx.fillStyle = grad;
      ctx.fillRect(x, y, 14, h);
      ctx.strokeStyle = cssVar("--border-strong");
      ctx.strokeRect(x, y, 14, h);
      ctx.fillStyle = cssVar("--text");
      ctx.font = `500 11px ${cssVar("--font")}`;
      for (const tick of legendTicks()) {
        const yy = y + h - tick.t * h;
        ctx.fillText(tick.label, x + 20, yy + 4);
      }
      ctx.fillText(d.unit, x - 4, y - 10);
      ctx.restore();
    }
    return c.toDataURL("image/png");
  }

  function legendColor(tt: number): string {
    const l = lut(store.colormap);
    const i = Math.round(tt * 255) * 4;
    return `rgb(${l[i]},${l[i + 1]},${l[i + 2]})`;
  }

  function legendTicks(): { t: number; label: string }[] {
    const d = store.display;
    if (!d) return [];
    const out: { t: number; label: string }[] = [];
    for (let k = 0; k <= 4; k++) {
      const tt = k / 4;
      const v = d.log ? Math.pow(10, Math.log10(d.min) + tt * (Math.log10(d.max) - Math.log10(d.min))) : d.min + tt * (d.max - d.min);
      out.push({ t: tt, label: sig(v, 3) });
    }
    return out;
  }
</script>

<div
  class="viewer"
  bind:this={container}
  role="application"
  aria-label={t("viewer.label")}
>
  <canvas bind:this={glCanvas} class="layer"></canvas>
  <canvas
    bind:this={ovCanvas}
    class="layer overlay"
    class:cut={store.tool === "cut" && showResult}
    onwheel={onWheel}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointerleave={() => (hover = null)}
    ondblclick={fit}
  ></canvas>

  <div class="toolbar">
    {#if store.summary}
      <div class="segmented">
        <button class:active={store.viewMode === "layout"} onclick={() => (store.viewMode = "layout")}>
          <Layers3 size={14} />{t("viewer.layoutView")}
        </button>
        <button class:active={store.viewMode === "result"} onclick={() => (store.viewMode = "result")}>
          <Flame size={14} />{t("viewer.resultView")}
        </button>
      </div>
    {/if}
    {#if showResult}
      <div class="segmented">
        <button class:active={store.tool === "pan"} title={t("viewer.pan")} onclick={() => (store.tool = "pan")}>
          <Hand size={14} />
        </button>
        <button class:active={store.tool === "cut"} title={t("viewer.cut")} onclick={() => (store.tool = "cut")}>
          <Ruler size={14} />
        </button>
      </div>
    {/if}
    <button class="btn sm icon tool-btn" title={t("viewer.fit")} onclick={fit}><Maximize size={14} /></button>
  </div>

  {#if showResult && store.display && store.display.kind !== "material"}
    {@const d = store.display}
    <div class="legend">
      <div class="unit">{d.unit}</div>
      <div class="bar-wrap">
        <div class="bar" style:background={cssGradient(store.colormap)}></div>
        <div class="ticks">
          {#each legendTicks() as tick (tick.t)}
            <span style:bottom="{tick.t * 100}%">{tick.label}</span>
          {/each}
        </div>
      </div>
      {#if d.log}<div class="unit muted">log</div>{/if}
    </div>
  {/if}

  {#if hover && (hover.lines.length || !showResult)}
    <div class="tooltip" style:left="{Math.min(hover.sx + 16, W - 220)}px" style:top="{Math.min(hover.sy + 16, H - 140)}px">
      <div class="coords num">x {sig(hover.x, 6)} µm · y {sig(hover.y, 6)} µm</div>
      {#each hover.lines as [k, v] (k + v)}
        <div class="line"><span>{k}</span><span class="num">{v}</span></div>
      {/each}
    </div>
  {/if}

  {#if glError}
    <div class="gl-error hint error">{glError}</div>
  {/if}
</div>

<style>
  .viewer {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background-color: var(--viewer-bg);
    background-image: radial-gradient(var(--viewer-grid) 1px, transparent 1px);
    background-size: 22px 22px;
  }
  .layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .overlay {
    cursor: grab;
    touch-action: none;
  }
  .overlay:active {
    cursor: grabbing;
  }
  .overlay.cut {
    cursor: crosshair;
  }
  .toolbar {
    position: absolute;
    top: 12px;
    left: 12px;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .toolbar .segmented {
    background: var(--panel);
    box-shadow: var(--shadow);
    border: 1px solid var(--border);
  }
  .toolbar .segmented button.active {
    background: var(--accent-weak);
    color: var(--accent-text);
    box-shadow: none;
  }
  .tool-btn {
    box-shadow: var(--shadow);
    height: 32px;
    width: 32px;
  }
  .legend {
    position: absolute;
    right: 14px;
    top: 14px;
    bottom: 60px;
    max-height: 340px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 10px 12px;
    background: color-mix(in srgb, var(--panel) 88%, transparent);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    backdrop-filter: blur(6px);
    pointer-events: none;
  }
  .legend .unit {
    font-size: 11px;
    font-weight: 600;
  }
  .bar-wrap {
    flex: 1;
    display: flex;
    gap: 6px;
    min-height: 120px;
  }
  .bar {
    width: 12px;
    border-radius: 3px;
    border: 1px solid var(--border-strong);
  }
  .ticks {
    position: relative;
    width: 56px;
  }
  .ticks span {
    position: absolute;
    left: 0;
    transform: translateY(50%);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    white-space: nowrap;
  }
  .tooltip {
    position: absolute;
    pointer-events: none;
    min-width: 170px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--panel) 94%, transparent);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    font-size: 11.5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tooltip .coords {
    color: var(--muted);
    margin-bottom: 2px;
  }
  .tooltip .line {
    display: flex;
    justify-content: space-between;
    gap: 14px;
  }
  .tooltip .line span:first-child {
    color: var(--muted);
  }
  .gl-error {
    position: absolute;
    left: 12px;
    bottom: 40px;
    right: 12px;
  }
</style>
