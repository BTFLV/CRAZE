<script lang="ts">
  import { X, Info, CircleCheck, TriangleAlert, CircleX } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
</script>

<div class="toasts">
  {#each store.toasts as toast (toast.id)}
    <div class="toast {toast.kind}">
      {#if toast.kind === "ok"}<CircleCheck size={16} />{:else if toast.kind === "warn"}<TriangleAlert size={16} />{:else if toast.kind === "error"}<CircleX
          size={16}
        />{:else}<Info size={16} />{/if}
      <span>{toast.text}</span>
      <button class="close" onclick={() => store.dismiss(toast.id)} aria-label="Schließen"><X size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 40px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 100;
    max-width: 420px;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    font-size: 12.5px;
    animation: slide 0.18s ease-out;
    white-space: pre-line;
  }
  .toast :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .toast.ok :global(svg) {
    color: var(--ok);
  }
  .toast.warn :global(svg) {
    color: var(--warn);
  }
  .toast.error :global(svg) {
    color: var(--danger);
  }
  .toast.info :global(svg) {
    color: var(--accent);
  }
  .toast span {
    flex: 1;
  }
  .close {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0;
  }
  .close :global(svg) {
    color: var(--muted) !important;
  }
  @keyframes slide {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }
</style>
