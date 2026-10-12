<script lang="ts">
  import { Play, Square, TriangleAlert, Lightbulb, ArrowRight, CircleCheck } from "@lucide/svelte";
  import { store } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n";
  import { duration, length, num, si } from "../../lib/format";
  import NumInput from "../NumInput.svelte";

  const PRESETS = [500, 1000, 2000, 3000, 4000];

  const size = $derived.by(() => {
    const b = store.layout?.bbox;
    if (!b) return null;
    // Simulated area: region layer if assigned, otherwise the layout bbox (approximation for the estimate).
    let w = b.max.x - b.min.x;
    let h = b.max.y - b.min.y;
    const region = store.setup?.layers.filter((a) => a.role.type === "region") ?? [];
    if (region.length) {
      const infos = store.layout!.layers.filter((l) => region.some((r) => r.key.layer === l.key.layer && r.key.datatype === l.key.datatype));
      if (infos.length) {
        w = Math.max(...infos.map((i) => i.bbox.max.x)) - Math.min(...infos.map((i) => i.bbox.min.x));
        h = Math.max(...infos.map((i) => i.bbox.max.y)) - Math.min(...infos.map((i) => i.bbox.min.y));
      }
    }
    return { w, h };
  });

  const px = $derived(size ? Math.max(size.w, size.h) / store.settings.resolution : 0);
  const nx = $derived(size ? Math.ceil(size.w / px) : 0);
  const ny = $derived(size ? Math.ceil(size.h / px) : 0);
  const unknowns = $derived.by(() => {
    if (!store.layout || !px) return 0;
    const area = store.layout.layers
      .filter((l) => store.conductorKeys.some((k) => k.layer === l.key.layer && k.datatype === l.key.datatype))
      .reduce((a, l) => a + l.areaUm2, 0);
    return Math.min(nx * ny, area / (px * px));
  });
  const recommended = $derived(store.recommendedResolution());
  const pxPerWidth = $derived(store.minConductorPathWidth && px ? store.minConductorPathWidth / px : null);
  const totalMs = $derived(
    store.summary
      ? store.summary.stats.timeRasterMs + store.summary.stats.timeBuildMs + store.summary.stats.timeSolveMs + store.summary.stats.timePostMs
      : 0,
  );
</script>

<div class="panel-header">
  <h2>{t("simulate.title")}</h2>
  <p>{t("simulate.desc")}</p>
</div>

<div class="section">
  <div class="section-title">{t("simulate.resolution")}</div>
  <div class="row">
    <input
      type="range"
      min="7"
      max="13"
      step="0.01"
      value={Math.log2(store.settings.resolution)}
      oninput={(e) => (store.settings.resolution = Math.round(Math.pow(2, Number(e.currentTarget.value)) / 50) * 50)}
    />
    <div style="width: 130px; flex: none;">
      <NumInput bind:value={store.settings.resolution} unit="px" min={64} max={8000} integer />
    </div>
  </div>
  <div class="presets">
    {#each PRESETS as p (p)}
      <button class="btn sm" class:sel={store.settings.resolution === p} onclick={() => (store.settings.resolution = p)}>{p}</button>
    {/each}
  </div>
  <dl class="kv">
    <dt>{t("simulate.grid")}</dt>
    <dd>{nx} × {ny} px</dd>
    <dt>{t("simulate.pixelSize")}</dt>
    <dd>{length(px)}</dd>
    {#if pxPerWidth}
      <dt>{store.lang === "de" ? "Pixel über schmalster Bahn" : "Pixels across narrowest trace"}</dt>
      <dd class:warn={pxPerWidth < 10}>{num(pxPerWidth, 3)}</dd>
    {/if}
    <dt>{store.lang === "de" ? "Unbekannte (ca.)" : "Unknowns (approx.)"}</dt>
    <dd>{num(Math.round(unknowns), 0)}</dd>
  </dl>
  {#if recommended && store.settings.resolution < recommended}
    <div class="hint info">
      <Lightbulb size={14} />
      <span>
        {t("simulate.recommended", { r: recommended, w: length(store.minConductorPathWidth ?? 0) })}
        <button class="btn sm" style="margin-top:6px" onclick={() => (store.settings.resolution = recommended)}>{t("simulate.useRecommended")}</button>
      </span>
    </div>
  {/if}
  {#if unknowns > 6e6}
    <div class="hint warn"><TriangleAlert size={14} /><span>{t("simulate.memoryWarn")}</span></div>
  {/if}

  <details>
    <summary>{t("simulate.advanced")}</summary>
    <div class="adv">
      <label class="field">
        <span>{t("simulate.solver")}</span>
        <select class="input" bind:value={store.settings.solver}>
          <option value="auto">{t("simulate.solverAuto")}</option>
          <option value="direct">{t("simulate.solverDirect")}</option>
          <option value="iterative">{t("simulate.solverIterative")}</option>
        </select>
      </label>
      {#if store.settings.solver === "iterative"}
        <div class="grid2">
          <label class="field"><span>{t("simulate.tolerance")}</span><NumInput bind:value={store.settings.tolerance} min={1e-16} max={1e-2} /></label>
          <label class="field"><span>{t("simulate.maxIter")}</span><NumInput bind:value={store.settings.maxIterations} min={10} integer /></label>
        </div>
      {/if}
    </div>
  </details>
</div>

<div class="section">
  {#if store.problems.length}
    <div class="section-title">{t("simulate.problems")}</div>
    {#each store.problems as p (p)}
      <div class="hint warn"><TriangleAlert size={14} /><span>{p}</span></div>
    {/each}
  {/if}
  {#if store.job?.kind === "simulate"}
    <div class="progress">
      <div class="msg">{store.job.message || t("simulate.running")}</div>
      <div class="bar" class:indet={store.job.fraction == null}>
        <div style:width="{(store.job.fraction ?? 0.35) * 100}%"></div>
      </div>
    </div>
    <button class="btn block big" onclick={() => store.cancel()}><Square size={15} />{t("simulate.cancel")}</button>
  {:else}
    <button class="btn primary block big" disabled={!!store.problems.length || !!store.job} onclick={() => store.simulate()}>
      <Play size={16} />{store.summary ? t("simulate.restart") : t("simulate.start")}
    </button>
  {/if}
  {#if store.lastError && !store.job}
    <div class="hint error"><TriangleAlert size={14} /><span style="white-space: pre-line">{store.lastError}</span></div>
  {/if}
  {#if store.summary && !store.job}
    <div class="hint ok">
      <CircleCheck size={14} />
      <span>
        {t("simulate.done", { t: duration(totalMs) })} · R = {si(store.summary.squares * store.sheetResistance, "Ω", 5)}
      </span>
    </div>
    <button class="btn block" onclick={() => (store.step = "results")}>{t("steps.results")}<ArrowRight size={15} /></button>
  {/if}
</div>

<style>
  .presets {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .presets .sel {
    border-color: var(--accent);
    color: var(--accent-text);
    background: var(--accent-weak);
  }
  .warn {
    color: var(--warn);
    font-weight: 600;
  }
  details summary {
    cursor: pointer;
    color: var(--text-2);
    font-weight: 500;
  }
  .adv {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 10px;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .msg {
    font-size: 12px;
    color: var(--text-2);
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--panel-3);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.2s;
  }
  .bar.indet div {
    animation: indet 1.2s ease-in-out infinite;
  }
  @keyframes indet {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(300%);
    }
  }
</style>
