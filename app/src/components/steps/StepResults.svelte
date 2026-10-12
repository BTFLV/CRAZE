<script lang="ts">
  import { Crosshair, Ruler, Download, Image, FileText, TriangleAlert, Gauge, Info } from "@lucide/svelte";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { store, type FieldKind } from "../../lib/store.svelte";
  import { materialName, t } from "../../lib/i18n";
  import { duration, num, sci, si, sig } from "../../lib/format";
  import { electrical, sheetResistance, volumeCurrentFactor } from "../../lib/physics";
  import { COLORMAPS, TERMINAL_COLORS } from "../../lib/colormaps";
  import { lineCurrent, sampleLine } from "../../lib/display";
  import { viewerApi } from "../../lib/viewer";
  import { buildReport } from "../../lib/report";
  import * as api from "../../lib/api";
  import NumInput from "../NumInput.svelte";
  import Chart from "../Chart.svelte";

  const s = $derived(store.summary!);
  const el = $derived(electrical(s.squares, store.sheetResistance, store.excitation.mode, store.excitation.value));
  const vol = $derived(volumeCurrentFactor(store.stack, store.temperature));
  const d = $derived(store.display);

  // Peak current density / power density in physical units (independent of the selected view).
  const peaks = $derived.by(() => {
    const f = store.fields;
    if (!f) return null;
    let best = 0;
    let bi = -1;
    for (let i = 0; i < f.material.length; i++) {
      if (f.material[i] !== 1) continue;
      const m = f.jx[i] * f.jx[i] + f.jy[i] * f.jy[i];
      if (m > best) {
        best = m;
        bi = i;
      }
    }
    const ks = el.voltage / store.sheetResistance / (s.grid.pxUm * 1e-6);
    const K = Math.sqrt(best) * ks;
    const g = s.grid;
    return {
      K,
      J: vol ? K * vol.factor : null,
      p: K * K * store.sheetResistance,
      x: g.x0Um + ((bi % g.nx) + 0.5) * g.pxUm,
      y: g.y0Um - (Math.floor(bi / g.nx) + 0.5) * g.pxUm,
    };
  });

  const floating = $derived(
    store.setup ? store.setup.terminals.map((_, i) => i).filter((i) => i !== store.setup!.source && i !== store.setup!.sink) : [],
  );

  // Cross-section
  const cutData = $derived.by(() => {
    const c = store.cut;
    if (!c || !d || !store.fields) return null;
    const sm = sampleLine(d.kind === "material" ? d.potentialV : d.values, s, c.a, c.b);
    const current = lineCurrent(store.fields, s, c.a, c.b, el.voltage, store.sheetResistance);
    return { ...sm, current, length: sm.d[sm.d.length - 1] };
  });
  const cutUnit = $derived(d && d.kind !== "material" ? d.unit : "V");

  // R(T)
  const rt = $derived.by(() => {
    const ts: number[] = [];
    const rs: number[] = [];
    for (let T = -50; T <= 400; T += 5) {
      ts.push(T);
      rs.push(s.squares * sheetResistance(store.stack, T));
    }
    return [ts, rs];
  });

  // Resolution study: Richardson extrapolation from the last three points.
  const extrap = $derived.by(() => {
    const p = store.study;
    if (p.length < 3) return null;
    const [a, b, c] = p.slice(-3);
    const d1 = a.squares - b.squares;
    const d2 = b.squares - c.squares;
    if (Math.abs(d2) < 1e-15 || d1 / d2 <= 1) return { value: c.squares, err: Math.abs(d2) / c.squares, order: null };
    const ratio = b.resolution / a.resolution;
    const order = Math.log(d1 / d2) / Math.log(ratio);
    const inf = c.squares - d2 / (Math.pow(c.resolution / b.resolution, order) - 1);
    return { value: inf, err: Math.abs(c.squares - inf) / inf, order };
  });

  const fieldOptions: { id: FieldKind; label: string }[] = $derived([
    { id: "potential", label: t("results.fieldPotential") },
    { id: "current", label: t("results.fieldCurrent") },
    { id: "power", label: t("results.fieldPower") },
    { id: "material", label: t("results.fieldMaterial") },
  ]);

  async function exportCsv() {
    if (!cutData || !store.cut) return;
    const path = await saveDialog({ defaultPath: store.lang === "de" ? "querschnitt.csv" : "cross-section.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    const sep = store.lang === "de" ? ";" : ",";
    const fmt = (v: number) => (store.lang === "de" ? String(v).replace(".", ",") : String(v));
    const c = store.cut;
    const lines = [`distance_um${sep}x_um${sep}y_um${sep}value_${cutUnit}`];
    cutData.d.forEach((dist, i) => {
      const tt = dist / (cutData.length || 1);
      const x = c.a[0] + (c.b[0] - c.a[0]) * tt;
      const y = c.a[1] + (c.b[1] - c.a[1]) * tt;
      lines.push([dist, x, y].map(fmt).join(sep) + sep + (cutData.v[i] == null ? "" : fmt(cutData.v[i]!)));
    });
    await api.writeTextFile(path, lines.join("\n"));
    store.toast("ok", t("common.saved"));
  }

  async function exportImage() {
    const path = await saveDialog({ defaultPath: "craze.png", filters: [{ name: "PNG", extensions: ["png"] }] });
    if (!path) return;
    await api.writeBase64File(path, viewerApi.snapshot());
    store.toast("ok", t("results.imageSaved"));
  }

  async function exportReport() {
    const base = store.layout?.fileName.replace(/\.(gds2?|gdsii|txt)$/i, "") ?? "craze";
    const path = await saveDialog({ defaultPath: `${base}_${store.lang === "de" ? "bericht" : "report"}.html`, filters: [{ name: "HTML", extensions: ["html"] }] });
    if (!path) return;
    const html = buildReport(viewerApi.snapshot(), el, peaks, extrap);
    await api.writeTextFile(path, html);
    await openPath(path).catch(() => {});
    store.toast("ok", t("results.reportSaved"));
  }
</script>

<div class="panel-header">
  <h2>{t("results.title")}</h2>
</div>

{#if store.summary}
  <div class="section">
    <div class="result-card">
      <div class="label">{t("results.resistance")} {t("results.at", { t: `${store.temperature} °C` })}</div>
      <div class="big num">{si(el.resistance, "Ω", 5)}</div>
      <div class="sub num" title={t("results.squaresHelp")}>
        {t("results.squares", { n: sig(s.squares, 6) })} · R□ = {si(store.sheetResistance, "Ω", 4)}
      </div>
    </div>
    {#if store.resultStale}
      <div class="hint warn"><TriangleAlert size={14} /><span>{t("results.stale")}</span></div>
    {/if}
    {#each s.warnings as w (w)}
      <div class="hint warn"><TriangleAlert size={14} /><span>{w}</span></div>
    {/each}
  </div>

  <div class="section">
    <div class="section-title">{t("results.operating")}</div>
    <div class="segmented block">
      <button
        class:active={store.excitation.mode === "voltage"}
        onclick={() => (store.excitation = { mode: "voltage", value: Number(el.voltage.toPrecision(4)) })}>{t("results.byVoltage")}</button
      >
      <button
        class:active={store.excitation.mode === "current"}
        onclick={() => (store.excitation = { mode: "current", value: Number(el.current.toPrecision(4)) })}>{t("results.byCurrent")}</button
      >
    </div>
    {#if store.excitation.mode === "voltage"}
      <label class="field"><span>{t("results.voltage")}</span><NumInput bind:value={store.excitation.value} unit="V" /></label>
    {:else}
      <label class="field">
        <span>{t("results.current")}</span>
        <NumInput value={store.excitation.value * 1e3} unit="mA" onchange={(v) => (store.excitation.value = v / 1e3)} />
      </label>
    {/if}
    <dl class="kv">
      <dt>{t("results.voltage")}</dt>
      <dd>{si(el.voltage, "V")}</dd>
      <dt>{t("results.current")}</dt>
      <dd>{si(el.current, "A")}</dd>
      <dt>{t("results.power")}</dt>
      <dd>{si(el.power, "W")}</dd>
      {#if peaks}
        {#if peaks.J != null && vol}
          <dt>{t("results.maxJ")} <span class="muted">({t("results.in", { layer: materialName(vol.layer) })})</span></dt>
          <dd>{sig(peaks.J / 1e6, 4)} A/mm²</dd>
        {/if}
        <dt>{t("results.maxK")}</dt>
        <dd>{si(peaks.K, "A/m")}</dd>
        <dt>{t("results.maxP")}</dt>
        <dd>{si(peaks.p / 1e6, "W/mm²")}</dd>
      {/if}
    </dl>
    {#if peaks}
      <button class="btn sm" onclick={() => store.requestFocus(peaks.x, peaks.y)}><Crosshair size={14} />{t("results.hotspot")}</button>
    {/if}
  </div>

  {#if store.setup && store.setup.terminals.length > 2}
    <div class="section">
      <div class="section-title">{t("results.terminals")}</div>
      <table class="data">
        <thead><tr><th></th><th>Terminal</th><th style="text-align:right">{t("results.potential")}</th></tr></thead>
        <tbody>
          {#each store.setup.terminals as term, i (i)}
            <tr>
              <td style="width:16px"><span class="dot" style:background={TERMINAL_COLORS[i % 8]}></span></td>
              <td>
                {term.name}
                {#if i === store.setup.source}<span class="badge">+</span>{:else if i === store.setup.sink}<span class="badge">−</span>{:else}<span
                    class="badge">{t("layers.floating")}</span
                  >{/if}
              </td>
              <td style="text-align:right">{si(s.terminalPotentials[i] * el.voltage, "V")}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if floating.length === 2}
        {@const dv = Math.abs(s.terminalPotentials[floating[0]] - s.terminalPotentials[floating[1]]) * el.voltage}
        <dl class="kv">
          <dt>ΔU ({store.setup.terminals[floating[0]].name} – {store.setup.terminals[floating[1]].name})</dt>
          <dd>{si(dv, "V")}</dd>
          <dt><b>{t("results.rSense")}</b></dt>
          <dd><b>{si(dv / el.current, "Ω", 5)}</b></dd>
        </dl>
        <div class="hint"><Info size={14} /><span>{t("results.fourWire")}</span></div>
      {/if}
    </div>
  {/if}

  <div class="section">
    <div class="section-title">{t("results.view")}</div>
    <div class="segmented block grid4">
      {#each fieldOptions as o (o.id)}
        <button class:active={store.field === o.id} onclick={() => {
          store.field = o.id;
          store.viewMode = "result";
        }}>{o.label}</button>
      {/each}
    </div>
    {#if store.field !== "material"}
      <div class="grid2">
        <label class="field">
          <span>{t("results.colormap")}</span>
          <select class="input sm" bind:value={store.colormap} onchange={() => store.persist()}>
            {#each COLORMAPS as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        {#if store.field !== "potential"}
          <label class="field">
            <span>{t("results.range")}</span>
            <select class="input sm" bind:value={store.rangeMode}>
              <option value="auto">{t("results.rangeAuto")}</option>
              <option value="full">{t("results.rangeFull")}</option>
            </select>
          </label>
        {/if}
      </div>
      {#if store.field !== "potential"}
        <label class="check"><input type="checkbox" bind:checked={store.logScale} />{t("results.log")}</label>
      {/if}
    {/if}
    <label class="check"><input type="checkbox" bind:checked={store.showArrows} />{t("results.arrows")}</label>
    <label class="check"><input type="checkbox" bind:checked={store.showIsolines} />{t("results.isolines")}</label>
    <label class="check"><input type="checkbox" bind:checked={store.showOutlines} />{t("results.outlines")}</label>
  </div>

  <div class="section">
    <div class="section-title">
      {t("results.cut")}
      <button class="btn sm" class:sel={store.tool === "cut"} onclick={() => {
        store.tool = store.tool === "cut" ? "pan" : "cut";
        store.viewMode = "result";
      }}><Ruler size={14} />{t("results.cutTool")}</button>
    </div>
    {#if cutData}
      <Chart
        data={[cutData.d, cutData.v]}
        series={[{ label: cutUnit }]}
        xLabel={`${t("results.distance")} [µm]`}
        yLabel={cutUnit}
        height={170}
      />
      <dl class="kv">
        <dt>{t("results.cutLength")}</dt>
        <dd>{num(cutData.length, 4)} µm</dd>
        <dt>{t("results.cutCurrent")}</dt>
        <dd>{si(Math.abs(cutData.current), "A")} <span class="muted">({num((Math.abs(cutData.current) / el.current) * 100, 3)} %)</span></dd>
      </dl>
      <button class="btn sm" onclick={exportCsv}><Download size={14} />{t("results.exportCsv")}</button>
    {:else}
      <div class="hint"><Info size={14} /><span>{t("results.cutHint")}</span></div>
    {/if}
  </div>

  <div class="section">
    <div class="section-title">{t("results.temperature")}</div>
    <Chart data={rt} series={[{ label: "R" }]} xLabel="T [°C]" yLabel="R [Ω]" height={160} markX={store.temperature} />
  </div>

  <div class="section">
    <div class="section-title">{t("results.accuracy")}</div>
    <p class="muted small">{t("results.accuracyHint")}</p>
    {#if store.study.length}
      <table class="data">
        <thead><tr><th>Pixel</th><th style="text-align:right">R</th><th style="text-align:right">{t("results.timing")}</th></tr></thead>
        <tbody>
          {#each store.study as p (p.resolution)}
            <tr>
              <td>{p.resolution}</td>
              <td style="text-align:right">{si(p.squares * store.sheetResistance, "Ω", 6)}</td>
              <td style="text-align:right">{duration(p.timeMs)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if extrap}
        <dl class="kv">
          <dt>{t("results.extrapolated")}</dt>
          <dd>{si(extrap.value * store.sheetResistance, "Ω", 6)}</dd>
          <dt>{t("results.errorEstimate", { r: store.study[store.study.length - 1].resolution })}</dt>
          <dd>{sig(extrap.err * 100, 2)} %</dd>
        </dl>
      {/if}
    {/if}
    {#if store.job?.kind === "study"}
      <button class="btn block" onclick={() => store.cancel()}>{t("simulate.cancel")}</button>
    {:else}
      <button class="btn block" disabled={!!store.job || store.problems.length > 0} onclick={() => store.runStudy()}>
        <Gauge size={15} />{t("results.study")}
      </button>
    {/if}
  </div>

  <div class="section">
    <div class="section-title">{t("results.export")}</div>
    <div class="row">
      <button class="btn sm" onclick={exportImage}><Image size={14} />{t("results.exportImage")}</button>
      <button class="btn sm" onclick={exportReport}><FileText size={14} />{t("results.exportReport")}</button>
    </div>
  </div>

  <div class="section">
    <details>
      <summary>{t("results.details")}</summary>
      <dl class="kv" style="margin-top:8px">
        <dt>{t("results.unknowns")}</dt>
        <dd>{num(s.stats.unknowns, 0)}</dd>
        <dt>{t("results.removed")}</dt>
        <dd>{num(s.stats.removedPixels, 0)} px</dd>
        <dt>{t("results.solver")}</dt>
        <dd>{s.stats.solver === "direct" ? t("simulate.solverDirect") : `${t("simulate.solverIterative")}, ${s.stats.iterations} it.`}</dd>
        <dt>{t("results.residual")}</dt>
        <dd>{sci(s.stats.relativeResidual, 1)}</dd>
        <dt>{t("results.balance")}</dt>
        <dd>{sci(s.stats.currentBalanceError, 1)}</dd>
        <dt>{t("results.timing")}</dt>
        <dd>{duration(s.stats.timeRasterMs + s.stats.timeBuildMs + s.stats.timeSolveMs + s.stats.timePostMs)}</dd>
        <dt class="muted">· Raster / Netz / Lösen</dt>
        <dd class="muted">{duration(s.stats.timeRasterMs)} / {duration(s.stats.timeBuildMs)} / {duration(s.stats.timeSolveMs)}</dd>
      </dl>
    </details>
  </div>
{/if}

<style>
  .result-card {
    padding: 14px 16px;
    border-radius: var(--radius);
    background: linear-gradient(135deg, var(--accent-weak), var(--panel-2));
    border: 1px solid var(--border);
  }
  .result-card .label {
    font-size: 12px;
    color: var(--text-2);
    font-weight: 500;
  }
  .result-card .big {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--accent-text);
    user-select: text;
    -webkit-user-select: text;
  }
  .result-card .sub {
    color: var(--muted);
    font-size: 12px;
  }
  .segmented.grid4 {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .sel {
    border-color: var(--accent);
    color: var(--accent-text);
    background: var(--accent-weak);
  }
  .small {
    font-size: 12px;
  }
  details summary {
    cursor: pointer;
    color: var(--text-2);
    font-weight: 500;
  }
</style>
