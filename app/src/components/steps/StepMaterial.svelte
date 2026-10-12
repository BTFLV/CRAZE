<script lang="ts">
  import { Plus, Trash2, ArrowRight, Info } from "@lucide/svelte";
  import { store } from "../../lib/store.svelte";
  import { materialName, t } from "../../lib/i18n";
  import { si, sig } from "../../lib/format";
  import { effectiveTcr, sheetResistance } from "../../lib/physics";
  import NumInput from "../NumInput.svelte";
  import type { Stack } from "../../lib/types";

  const CUSTOM = "__custom";

  function setMode(mode: "layers" | "measured") {
    if (store.stack.mode === mode) return;
    if (mode === "measured") {
      const rs = sheetResistance(store.stack, 20);
      store.stack = { mode: "measured", sheetResistance: isFinite(rs) ? Number(rs.toPrecision(4)) : 1, tcr: 3.85e-3, tRefC: 20 };
    } else {
      store.stack = {
        mode: "layers",
        layers: [{ material: { ...(store.materials.find((m) => m.name === "Platin") ?? store.materials[0]) }, thicknessNm: 100 }],
      };
    }
  }

  function pickMaterial(i: number, name: string) {
    if (store.stack.mode !== "layers") return;
    const l = store.stack.layers[i];
    if (name === CUSTOM) {
      l.material = { ...l.material, name: store.lang === "de" ? "Eigenes Material" : "Custom material" };
      return;
    }
    const m = store.materials.find((x) => x.name === name);
    if (m) l.material = { ...m };
  }

  function addLayer() {
    if (store.stack.mode !== "layers") return;
    const m = store.materials.find((x) => x.name === "Gold") ?? store.materials[0];
    store.stack.layers.push({ material: { ...m }, thicknessNm: 50 });
  }

  const isLib = (name: string) => store.materials.some((m) => m.name === name);

  // TCR is edited in ppm/K for readability.
  function ppm(s: Stack, i: number | null): number {
    if (s.mode === "measured") return s.tcr * 1e6;
    return (s.layers[i!]?.material.tcr ?? 0) * 1e6;
  }

  const rsRef = $derived(sheetResistance(store.stack, 20));
  const tcrEff = $derived(effectiveTcr(store.stack, 20));
</script>

<div class="panel-header">
  <h2>{t("material.title")}</h2>
  <p>{t("material.desc")}</p>
</div>

<div class="section">
  <div class="segmented block">
    <button class:active={store.stack.mode === "layers"} onclick={() => setMode("layers")}>{t("material.modeLayers")}</button>
    <button class:active={store.stack.mode === "measured"} onclick={() => setMode("measured")}>{t("material.modeMeasured")}</button>
  </div>

  {#if store.stack.mode === "layers"}
    {#each store.stack.layers as layer, i (i)}
      <div class="card stack-layer">
        <div class="row">
          <select
            class="input sm"
            value={isLib(layer.material.name) ? layer.material.name : CUSTOM}
            onchange={(e) => pickMaterial(i, e.currentTarget.value)}
          >
            {#each store.materials as m (m.name)}<option value={m.name}>{materialName(m.name)}</option>{/each}
            <option value={CUSTOM}>{t("material.custom")} …</option>
          </select>
          {#if store.stack.layers.length > 1}
            <button class="btn ghost sm icon" title={t("layers.remove")} onclick={() => store.stack.mode === "layers" && store.stack.layers.splice(i, 1)}>
              <Trash2 size={14} />
            </button>
          {/if}
        </div>
        {#if !isLib(layer.material.name)}
          <label class="field"><span>{t("material.name")}</span><input class="input sm" bind:value={layer.material.name} /></label>
        {/if}
        <div class="grid2">
          <label class="field"><span>{t("material.thickness")}</span><NumInput size="sm" bind:value={layer.thicknessNm} unit="nm" min={0} /></label>
          <label class="field">
            <span>α (TCR)</span>
            <NumInput size="sm" value={ppm(store.stack, i)} unit="ppm/K" onchange={(v) => (layer.material.tcr = v / 1e6)} />
          </label>
        </div>
        <label class="field"><span>{t("material.resistivity")} (20 °C)</span><NumInput size="sm" bind:value={layer.material.resistivity} unit="Ω·m" min={0} /></label>
        <div class="muted small">R□ = {si(layer.material.resistivity / (layer.thicknessNm * 1e-9), "Ω")}</div>
      </div>
    {/each}
    <button class="btn sm" onclick={addLayer}><Plus size={14} />{t("material.addLayer")}</button>
    {#if store.stack.layers.length > 1}
      <div class="hint"><Info size={14} /><span>{t("material.parallelHint")}</span></div>
    {/if}
    <div class="hint"><Info size={14} /><span>{t("material.bulkHint")}</span></div>
  {:else}
    <div class="grid2">
      <label class="field"><span>{t("material.sheet")}</span><NumInput bind:value={store.stack.sheetResistance} unit="Ω/□" min={0} /></label>
      <label class="field"><span>{t("material.tref")}</span><NumInput bind:value={store.stack.tRefC} unit="°C" /></label>
    </div>
    <label class="field">
      <span>{t("material.tcr")}</span>
      <NumInput value={ppm(store.stack, null)} unit="ppm/K" onchange={(v) => store.stack.mode === "measured" && (store.stack.tcr = v / 1e6)} />
    </label>
  {/if}
</div>

<div class="section">
  <label class="field"><span>{t("material.temperature")}</span><NumInput bind:value={store.temperature} unit="°C" min={-273} /></label>
  <dl class="kv">
    <dt>{t("material.sheetAt", { t: "20 °C" })}</dt>
    <dd>{si(rsRef, "Ω")}</dd>
    {#if store.temperature !== 20}
      <dt>{t("material.sheetAt", { t: `${store.temperature} °C` })}</dt>
      <dd><b>{si(store.sheetResistance, "Ω")}</b></dd>
    {/if}
    <dt>{t("material.effTcr")}</dt>
    <dd>{sig(tcrEff * 1e6, 4)} ppm/K</dd>
  </dl>
</div>

<div class="section">
  <button class="btn primary block big" onclick={() => (store.step = "simulate")}>
    {t("material.next")}<ArrowRight size={16} />
  </button>
</div>

<style>
  .stack-layer {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .stack-layer .row select {
    flex: 1;
  }
  .small {
    font-size: 11px;
  }
</style>
