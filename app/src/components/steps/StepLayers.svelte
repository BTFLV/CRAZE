<script lang="ts">
  import { Eye, EyeOff, Plus, Trash2, ArrowRight, Bookmark, TriangleAlert, Info } from "@lucide/svelte";
  import { store } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n";
  import { TERMINAL_COLORS, layerColor } from "../../lib/colormaps";
  import { keyStr, type LayerKey, type Role } from "../../lib/types";
  import { length, num } from "../../lib/format";

  function roleValue(r: Role): string {
    return r.type === "terminal" ? `t${r.id}` : r.type;
  }

  function onRole(key: LayerKey, v: string) {
    const role: Role = v.startsWith("t") && v !== "terminal" ? { type: "terminal", id: Number(v.slice(1)) } : ({ type: v } as Role);
    store.setRole(key, role);
  }

  function swatch(key: LayerKey, i: number): string {
    const r = store.roleOf(key);
    if (r.type === "conductor") return "#8f9bb3";
    if (r.type === "terminal") return TERMINAL_COLORS[r.id % 8];
    if (r.type === "region") return "transparent";
    return layerColor(i);
  }

  function setSource(id: number) {
    const s = store.setup!;
    if (s.sink === id) s.sink = s.source;
    s.source = id;
  }

  function setSink(id: number) {
    const s = store.setup!;
    if (s.source === id) s.source = s.sink;
    s.sink = id;
  }

  function savePreset() {
    store.savePreset();
    store.toast("ok", t("layers.presetSaved"));
  }
</script>

<div class="panel-header">
  <h2>{t("layers.title")}</h2>
  <p>{t("layers.desc")}</p>
</div>

{#if store.layout && store.setup}
  <div class="section">
    <div class="layers">
      {#each store.layout.layers as l, i (keyStr(l.key))}
        {@const hidden = store.hiddenLayers.includes(keyStr(l.key))}
        {@const role = store.roleOf(l.key)}
        <div class="layer" class:dim={role.type === "ignore"}>
          <button class="btn ghost sm icon" title={t("layers.show")} onclick={() => store.toggleLayer(l.key)}>
            {#if hidden}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
          </button>
          <span class="sw" class:region={role.type === "region"} style:background={swatch(l.key, i)}></span>
          <div class="info">
            <span class="key num">{keyStr(l.key)}</span>
            <span class="meta">
              {t(l.shapes === 1 ? "layers.shape" : "layers.shapes", { n: l.shapes })}{#if l.paths}, {t(l.paths === 1 ? "layers.path" : "layers.paths", { n: l.paths })}{/if}
              {#if l.minPathWidthUm} · {t("layers.minWidth", { w: length(l.minPathWidthUm) })}{/if}
            </span>
          </div>
          <select class="input sm role" value={roleValue(role)} onchange={(e) => onRole(l.key, e.currentTarget.value)}>
            <option value="ignore">{t("layers.roleIgnore")}</option>
            <option value="conductor">{t("layers.roleConductor")}</option>
            {#each store.setup.terminals as term, id (id)}
              <option value={`t${id}`}>{t("layers.terminal")}: {term.name}</option>
            {/each}
            <option value="region">{t("layers.roleRegion")}</option>
          </select>
        </div>
      {/each}
    </div>
    <div class="row">
      <button class="btn sm" onclick={savePreset}><Bookmark size={14} />{t("layers.savePreset")}</button>
    </div>
  </div>

  <div class="section">
    <div class="section-title">
      {t("layers.terminals")}
      <button class="btn ghost sm" onclick={() => store.addTerminal()}><Plus size={14} />{t("layers.addTerminal")}</button>
    </div>
    {#each store.setup.terminals as term, id (id)}
      <div class="terminal">
        <span class="dot" style:background={TERMINAL_COLORS[id % 8]}></span>
        <input class="input sm" bind:value={term.name} />
        <div class="segmented">
          <button class:active={store.setup.source === id} onclick={() => setSource(id)}>+ {t("layers.plus")}</button>
          <button class:active={store.setup.sink === id} onclick={() => setSink(id)}>− {t("layers.minus")}</button>
        </div>
        {#if store.setup.terminals.length > 2}
          <button class="btn ghost sm icon" title={t("layers.remove")} onclick={() => store.removeTerminal(id)}><Trash2 size={14} /></button>
        {/if}
      </div>
    {/each}
    <div class="hint"><Info size={14} /><span>{t("layers.terminalsHint")}</span></div>
    {#if !store.setup.layers.some((a) => a.role.type === "region")}
      <div class="hint"><Info size={14} /><span>{t("layers.regionHint")}</span></div>
    {/if}
  </div>

  {#if store.layout.labels.length}
    <div class="section">
      <div class="section-title">{t("layers.labels")}</div>
      <div class="labels">
        {#each store.layout.labels.slice(0, 20) as lb, i (i)}
          <span class="badge" title={`${keyStr(lb.key)} @ ${num(lb.x)}, ${num(lb.y)} µm`}>{lb.text}</span>
        {/each}
      </div>
    </div>
  {/if}

  <div class="section">
    {#each store.problems as p (p)}
      <div class="hint warn"><TriangleAlert size={14} /><span>{p}</span></div>
    {/each}
    <button class="btn primary block big" onclick={() => (store.step = "material")}>
      {t("layers.next")}<ArrowRight size={16} />
    </button>
  </div>
{/if}

<style>
  .layers {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .layer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px 5px 2px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }
  .layer.dim .info {
    opacity: 0.6;
  }
  .sw {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 4px;
  }
  .sw.region {
    border: 2px dashed var(--accent);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .key {
    font-weight: 600;
  }
  .meta {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .role {
    width: 150px;
    flex: none;
  }
  .terminal {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .terminal .input {
    flex: 1;
  }
  .dot {
    flex: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .segmented button.active {
    color: var(--accent-text);
  }
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
</style>
