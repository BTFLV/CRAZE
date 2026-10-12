<script lang="ts">
  import { Target, Square, Save, TriangleAlert, CircleCheck, Info, FolderOpen } from "@lucide/svelte";
  import { store } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n";
  import { length, num, si, sig } from "../../lib/format";
  import NumInput from "../NumInput.svelte";
  import Chart from "../Chart.svelte";

  let savedPath = $state<string | null>(null);

  const chart = $derived([store.optSteps.map((s) => s.iteration), store.optSteps.map((s) => s.resistance)]);
  const minW = $derived(store.minConductorPathWidth);

  async function save() {
    const p = await store.saveOptimized();
    if (p) savedPath = p;
  }
</script>

<div class="panel-header">
  <h2>{t("optimize.title")}</h2>
  <p>{t("optimize.desc")}</p>
</div>

<div class="section">
  <label class="field"><span>{t("optimize.target")}</span><NumInput bind:value={store.optimize.target} unit="Ω" min={1e-6} /></label>
  <div class="grid2">
    <label class="field"><span>{t("optimize.tolerance")}</span><NumInput bind:value={store.optimize.tolerancePct} unit="%" min={0.001} max={50} /></label>
    <label class="field"><span>{t("optimize.maxIter")}</span><NumInput bind:value={store.optimize.maxIterations} min={1} max={50} integer /></label>
  </div>
  <div class="hint">
    <Info size={14} />
    <span>{t("optimize.atTemp", { t: `${store.temperature} °C` })} R□ = {si(store.sheetResistance, "Ω")}, {store.settings.resolution} px.</span>
  </div>
  {#if !store.conductorHasPaths}
    <div class="hint warn"><TriangleAlert size={14} /><span>{t("optimize.noPaths")}</span></div>
  {/if}
  {#each store.problems as p (p)}
    <div class="hint warn"><TriangleAlert size={14} /><span>{p}</span></div>
  {/each}
  {#if store.job?.kind === "optimize"}
    <div class="muted small">{store.job.message}</div>
    <button class="btn block big" onclick={() => store.cancel()}><Square size={15} />{t("simulate.cancel")}</button>
  {:else}
    <button
      class="btn primary block big"
      disabled={!!store.job || !store.conductorHasPaths || store.problems.length > 0}
      onclick={() => {
        savedPath = null;
        store.runOptimization();
      }}
    >
      <Target size={16} />{t("optimize.start")}
    </button>
  {/if}
</div>

{#if store.optSteps.length}
  <div class="section">
    <Chart
      data={chart}
      series={[{ label: "R" }]}
      xLabel={t("optimize.step")}
      yLabel="R [Ω]"
      height={160}
      points
      integerX
      markY={store.optimize.target}
    />
    <table class="data">
      <thead><tr><th>{t("optimize.step")}</th><th style="text-align:right">{t("optimize.scale")}</th><th style="text-align:right">R</th></tr></thead>
      <tbody>
        {#each store.optSteps as s (s.iteration)}
          <tr>
            <td>{s.iteration}</td>
            <td style="text-align:right">{sig(s.scale, 5)}</td>
            <td style="text-align:right">{si(s.resistance, "Ω", 5)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

{#if store.optResult}
  {@const r = store.optResult}
  <div class="section">
    <div class="hint {r.converged ? 'ok' : 'warn'}">
      {#if r.converged}<CircleCheck size={14} />{:else}<TriangleAlert size={14} />{/if}
      <span>{r.converged ? t("optimize.converged") : t("optimize.notConverged")}</span>
    </div>
    <dl class="kv">
      <dt>{t("optimize.scale")}</dt>
      <dd>{sig(r.bestScale, 5)}</dd>
      <dt>R</dt>
      <dd><b>{si(r.bestResistance, "Ω", 5)}</b> ({num(((r.bestResistance - store.optimize.target) / store.optimize.target) * 100, 3)} %)</dd>
      {#if minW}
        <dt>{t("optimize.newWidth")}</dt>
        <dd>{length(minW)} → {length(minW * r.bestScale)}</dd>
      {/if}
    </dl>
    <button class="btn primary block" onclick={save}><Save size={15} />{t("optimize.saveGds")}</button>
    {#if savedPath}
      <button class="btn block" onclick={() => savedPath && store.openLayoutPath(savedPath)}><FolderOpen size={15} />{t("optimize.openSaved")}</button>
    {/if}
  </div>
{/if}

<style>
  .small {
    font-size: 12px;
  }
</style>
