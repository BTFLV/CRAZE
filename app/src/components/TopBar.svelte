<script lang="ts">
  import { FilePlus2, FolderOpen, Save, CircleHelp, Sun, Moon, Monitor, Languages } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import Logo from "./Logo.svelte";

  const projectName = $derived(
    store.projectPath ? store.projectPath.split(/[\\/]/).pop() : store.layout ? store.layout.fileName : "",
  );

  function cycleTheme() {
    const order = ["system", "light", "dark"] as const;
    store.setTheme(order[(order.indexOf(store.theme) + 1) % 3]);
  }
</script>

<header class="topbar">
  <div class="brand">
    <Logo size={26} />
    <span class="name">CRAZE</span>
    {#if projectName}
      <span class="sep">/</span>
      <span class="project" title={store.projectPath ?? store.layout?.path}>{projectName}</span>
      {#if store.dirty}<span class="dot" title={t("app.unsaved")}></span>{/if}
    {:else}
      <span class="tagline">{t("app.tagline")}</span>
    {/if}
  </div>
  <div class="actions">
    <button class="btn ghost sm" onclick={() => store.newProject()} disabled={!store.layout}>
      <FilePlus2 size={15} />{t("app.newProject")}
    </button>
    <button class="btn ghost sm" onclick={() => store.chooseFile()}>
      <FolderOpen size={15} />{t("app.open")}
    </button>
    <button class="btn ghost sm" onclick={() => store.saveProject()} disabled={!store.layout}>
      <Save size={15} />{t("app.save")}
    </button>
    <div class="divider"></div>
    <button
      class="btn ghost sm"
      title={t("app.language")}
      onclick={() => store.setLang(store.lang === "de" ? "en" : "de")}
    >
      <Languages size={15} />{store.lang.toUpperCase()}
    </button>
    <button class="btn ghost sm icon" title={`${t("app.theme")}: ${t(`app.theme${store.theme[0].toUpperCase()}${store.theme.slice(1)}`)}`} onclick={cycleTheme}>
      {#if store.theme === "light"}<Sun size={15} />{:else if store.theme === "dark"}<Moon size={15} />{:else}<Monitor size={15} />{/if}
    </button>
    <button class="btn ghost sm" onclick={() => (store.showHelp = true)}>
      <CircleHelp size={15} />{t("app.help")}
    </button>
  </div>
</header>

<style>
  .topbar {
    grid-area: top;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px 0 14px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }
  .name {
    font-weight: 700;
    letter-spacing: 0.04em;
    font-size: 14px;
  }
  .tagline {
    color: var(--muted);
    margin-left: 6px;
  }
  .sep {
    color: var(--border-strong);
  }
  .project {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .divider {
    width: 1px;
    height: 20px;
    background: var(--border);
    margin: 0 6px;
  }
</style>
