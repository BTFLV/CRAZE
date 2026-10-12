<script lang="ts">
  import { FileInput, Layers, Atom, Play, ChartColumn, Target, Check } from "@lucide/svelte";
  import { store, type Step } from "../lib/store.svelte";
  import { t } from "../lib/i18n";

  const steps: { id: Step; icon: typeof FileInput }[] = [
    { id: "layout", icon: FileInput },
    { id: "layers", icon: Layers },
    { id: "material", icon: Atom },
    { id: "simulate", icon: Play },
    { id: "results", icon: ChartColumn },
    { id: "optimize", icon: Target },
  ];

  function enabled(id: Step): boolean {
    if (id === "layout") return true;
    if (!store.layout) return false;
    if (id === "results") return !!store.summary;
    return true;
  }

  function done(id: Step): boolean {
    switch (id) {
      case "layout":
        return !!store.layout;
      case "layers":
        return !!store.setup && store.problems.length === 0;
      case "material":
        return !!store.layout && isFinite(store.sheetResistance);
      case "simulate":
        return !!store.summary && !store.resultStale;
      case "results":
        return false;
      case "optimize":
        return !!store.optResult?.converged;
    }
  }
</script>

<nav class="stepper">
  {#each steps as s, i (s.id)}
    {@const Icon = s.icon}
    <button
      class="step"
      class:active={store.step === s.id}
      class:done={done(s.id)}
      disabled={!enabled(s.id)}
      onclick={() => (store.step = s.id)}
    >
      <span class="num">
        {#if done(s.id) && store.step !== s.id}<Check size={14} strokeWidth={3} />{:else}<Icon size={15} />{/if}
      </span>
      <span class="text">
        <span class="title">{i + 1}. {t(`steps.${s.id}`)}</span>
        <span class="sub">{t(`steps.${s.id}Sub`)}</span>
      </span>
    </button>
  {/each}
</nav>

<style>
  .stepper {
    grid-area: nav;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 10px;
    border-right: 1px solid var(--border);
    background: var(--panel);
    overflow-y: auto;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--text);
  }
  .step:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .step:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .step.active {
    background: var(--accent-weak);
  }
  .num {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--panel-3);
    color: var(--text-2);
  }
  .step.active .num {
    background: var(--accent);
    color: #fff;
  }
  .step.done:not(.active) .num {
    background: var(--ok-weak);
    color: var(--ok);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 13px;
  }
  .step.active .title {
    color: var(--accent-text);
  }
  .sub {
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
