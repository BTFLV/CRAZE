<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { Upload } from "@lucide/svelte";
  import { store } from "./lib/store.svelte";
  import { t } from "./lib/i18n";
  import { buildDisplay, jMagnitude } from "./lib/display";
  import { volumeCurrentFactor } from "./lib/physics";
  import type { Fields } from "./lib/types";
  import TopBar from "./components/TopBar.svelte";
  import Stepper from "./components/Stepper.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import Viewer from "./components/Viewer.svelte";
  import Welcome from "./components/Welcome.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Help from "./components/Help.svelte";
  import StepLayout from "./components/steps/StepLayout.svelte";
  import StepLayers from "./components/steps/StepLayers.svelte";
  import StepMaterial from "./components/steps/StepMaterial.svelte";
  import StepSimulate from "./components/steps/StepSimulate.svelte";
  import StepResults from "./components/steps/StepResults.svelte";
  import StepOptimize from "./components/steps/StepOptimize.svelte";

  let dragOver = $state(false);
  let panel = $state<HTMLElement | null>(null);

  $effect(() => {
    void store.step;
    if (panel) panel.scrollTop = 0;
  });

  onMount(() => {
    store.init();
    let unlisten: (() => void) | undefined;
    Promise.resolve()
      .then(() => getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dragOver = true;
        else if (p.type === "leave") dragOver = false;
        else if (p.type === "drop") {
          dragOver = false;
          if (p.paths.length) store.openAny(p.paths[0]);
        }
      }))
      .then((u) => (unlisten = u))
      .catch(() => {});

    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      if (e.key === "o") {
        e.preventDefault();
        store.chooseFile();
      } else if (e.key === "s") {
        e.preventDefault();
        store.saveProject(e.shiftKey);
      } else if (e.key === "Enter") {
        e.preventDefault();
        store.simulate();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten?.();
      window.removeEventListener("keydown", onKey);
    };
  });

  // |j| is cached per result; the displayed field is rebuilt when view or operating point change.
  let jmagFor: Fields | null = null;
  let jmag: Float32Array | null = null;
  $effect(() => {
    const f = store.fields;
    const s = store.summary;
    if (!f || !s) {
      store.display = null;
      return;
    }
    if (jmagFor !== f) {
      jmag = jMagnitude(f);
      jmagFor = f;
    }
    const rs = store.sheetResistance;
    const voltage =
      store.excitation.mode === "voltage" ? store.excitation.value : store.excitation.value * s.squares * rs;
    const vol = volumeCurrentFactor(store.stack, store.temperature);
    store.display = buildDisplay(f, jmag!, s, {
      kind: store.field,
      voltage,
      rs,
      volumeFactor: vol?.factor ?? null,
      log: store.logScale,
      range: store.rangeMode,
    });
  });

  $effect(() => {
    document.documentElement.lang = store.lang;
  });
</script>

<div class="app" class:no-layout={!store.layout}>
  <TopBar />
  <Stepper />
  <main class="center">
    {#if store.layout}
      <Viewer />
    {:else}
      <Welcome />
    {/if}
  </main>
  {#if store.layout}
    <aside class="panel" bind:this={panel}>
      {#if store.step === "layout"}<StepLayout />
      {:else if store.step === "layers"}<StepLayers />
      {:else if store.step === "material"}<StepMaterial />
      {:else if store.step === "simulate"}<StepSimulate />
      {:else if store.step === "results" && store.summary}<StepResults />
      {:else if store.step === "optimize"}<StepOptimize />
      {:else}<StepLayout />{/if}
    </aside>
  {/if}
  <StatusBar />
</div>

<Toasts />
{#if store.showHelp}<Help />{/if}
{#if dragOver}
  <div class="drop-overlay">
    <div><Upload size={36} /><span>{t("welcome.drop").replace(/ oder$| or$/, "")}</span></div>
  </div>
{/if}

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 228px minmax(0, 1fr) 392px;
    grid-template-rows: 48px minmax(0, 1fr) 28px;
    grid-template-areas:
      "top top top"
      "nav center panel"
      "status status status";
  }
  .app.no-layout {
    grid-template-areas:
      "top top top"
      "nav center center"
      "status status status";
  }
  .center {
    grid-area: center;
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  .panel {
    grid-area: panel;
    border-left: 1px solid var(--border);
    background: var(--panel);
    overflow-y: auto;
    min-height: 0;
  }
  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border: 3px dashed var(--accent);
    pointer-events: none;
  }
  .drop-overlay div {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 26px 34px;
    border-radius: 16px;
    background: var(--panel);
    box-shadow: var(--shadow-lg);
    color: var(--accent-text);
    font-weight: 600;
    font-size: 15px;
  }
</style>
