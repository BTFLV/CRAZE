<script lang="ts">
  import { LoaderCircle } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import { length, si } from "../lib/format";
</script>

<footer class="status">
  <div class="left">
    {#if store.job}
      <LoaderCircle size={13} class="spin" />
      <span>{store.job.message || t("simulate.running")}</span>
      {#if store.job.fraction != null}
        <div class="bar"><div style:width="{store.job.fraction * 100}%"></div></div>
      {/if}
    {:else if store.busyLoading}
      <LoaderCircle size={13} class="spin" /><span>{t("status.loading")}</span>
    {:else}
      <span class="muted">{t("status.ready")}</span>
    {/if}
  </div>
  <div class="right num">
    {#if store.summary}
      <span>{t("status.pixels", { nx: store.summary.grid.nx, ny: store.summary.grid.ny, px: length(store.summary.grid.pxUm) })}</span>
    {/if}
    {#if store.layout}
      <span>R□ = {si(store.sheetResistance, "Ω")}</span>
      <span>{store.temperature} °C</span>
    {/if}
    <span class="muted">v2.0</span>
  </div>
</footer>

<style>
  .status {
    grid-area: status;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .left {
    gap: 8px;
  }
  .bar {
    width: 120px;
    height: 4px;
    border-radius: 2px;
    background: var(--panel-3);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
