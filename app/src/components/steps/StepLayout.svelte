<script lang="ts">
  import { FolderOpen, RotateCw, ArrowRight, FileBox } from "@lucide/svelte";
  import { store } from "../../lib/store.svelte";
  import { t } from "../../lib/i18n";
  import { length, sci } from "../../lib/format";

  const l = $derived(store.layout);
</script>

<div class="panel-header">
  <h2>{t("layout.title")}</h2>
  <p>{t("layout.desc")}</p>
</div>

{#if l}
  <div class="section">
    <dl class="kv">
      <dt>{t("layout.file")}</dt>
      <dd title={l.path} class="ellipsis">{l.fileName}</dd>
      <dt>{t("layout.size")}</dt>
      <dd>{length(l.bbox.max.x - l.bbox.min.x)} × {length(l.bbox.max.y - l.bbox.min.y)}</dd>
      <dt>{t("layout.layers")}</dt>
      <dd>{l.layers.length}</dd>
      <dt>{t("layout.shapes")}</dt>
      <dd>{l.layers.reduce((a, x) => a + x.shapes, 0)}</dd>
      <dt>{t("layout.unit")}</dt>
      <dd>{sci(l.dbUnitM, 0)} m</dd>
    </dl>
    {#if l.topCells.length > 1}
      <label class="field">
        <span>{t("layout.topCell")}</span>
        <select class="input" value={l.topCell} onchange={(e) => store.selectTopCell(e.currentTarget.value)}>
          {#each l.topCells as c (c)}<option value={c}>{c}</option>{/each}
        </select>
      </label>
      <div class="hint info">{t("layout.topCellHint")}</div>
    {/if}
    <div class="row">
      <button class="btn" onclick={() => store.chooseFile()}><FolderOpen size={15} />{t("layout.other")}</button>
      <button class="btn" onclick={() => store.reloadLayout()} title={t("layout.reload")}><RotateCw size={15} />{t("layout.reload")}</button>
    </div>
  </div>
  <div class="section">
    <button class="btn primary block big" onclick={() => (store.step = "layers")}>
      {t("layout.next")}<ArrowRight size={16} />
    </button>
  </div>
{:else}
  <div class="section">
    <button class="btn primary block big" onclick={() => store.chooseFile()}>
      <FolderOpen size={16} />{t("app.openLayout")}
    </button>
  </div>
{/if}

{#if store.examples.length}
  <div class="section">
    <div class="section-title">{t("welcome.examples")}</div>
    <div class="examples">
      {#each store.examples as ex (ex.path)}
        <button class="btn ghost sm ex" onclick={() => store.openLayoutPath(ex.path)}><FileBox size={14} />{ex.name}</button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .examples {
    display: flex;
    flex-direction: column;
    align-items: stretch;
  }
  .ex {
    justify-content: flex-start;
  }
</style>
