<script lang="ts">
  import { Upload, FileBox, FolderClock, Sparkles } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import Logo from "./Logo.svelte";
</script>

<div class="welcome">
  <div class="hero">
    <Logo size={64} />
    <h1>{t("welcome.title")}</h1>
    <p>{t("welcome.text")}</p>
  </div>

  <button class="drop" onclick={() => store.chooseFile()}>
    <Upload size={28} />
    <span><b>{t("welcome.drop")}</b> <u>{t("welcome.choose")}</u></span>
    <small>{t("welcome.formats")}</small>
  </button>

  <div class="cols">
    <section>
      <h3><Sparkles size={15} />{t("welcome.how")}</h3>
      <ol>
        <li>{t("welcome.how1")}</li>
        <li>{t("welcome.how2")}</li>
        <li>{t("welcome.how3")}</li>
      </ol>
    </section>
    {#if store.examples.length}
      <section>
        <h3><FileBox size={15} />{t("welcome.examples")}</h3>
        <div class="list">
          {#each store.examples as ex (ex.path)}
            <button class="item" onclick={() => store.openLayoutPath(ex.path)}>{ex.name}</button>
          {/each}
        </div>
      </section>
    {/if}
    {#if store.recent.length}
      <section>
        <h3><FolderClock size={15} />{t("welcome.recent")}</h3>
        <div class="list">
          {#each store.recent as r (r.path)}
            <button class="item" title={r.path} onclick={() => store.openAny(r.path)}>
              <span>{r.name}</span>
              <span class="badge">{r.kind === "project" ? t("welcome.project") : "GDS"}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .welcome {
    height: 100%;
    overflow-y: auto;
    padding: 48px 40px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 28px;
  }
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 12px;
    max-width: 620px;
  }
  .hero h1 {
    font-size: 26px;
    margin-top: 6px;
  }
  .hero p {
    color: var(--text-2);
    font-size: 14px;
  }
  .drop {
    width: min(620px, 100%);
    padding: 34px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    border: 2px dashed var(--border-strong);
    border-radius: 16px;
    background: var(--panel);
    cursor: pointer;
    color: var(--text);
    font-size: 14px;
    transition: border-color 0.15s, background 0.15s;
  }
  .drop:hover {
    border-color: var(--accent);
    background: var(--accent-weak);
  }
  .drop :global(svg) {
    color: var(--accent);
  }
  .drop u {
    color: var(--accent-text);
    text-decoration: none;
    font-weight: 600;
  }
  .drop small {
    color: var(--muted);
    font-size: 12px;
  }
  .cols {
    width: min(960px, 100%);
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
  }
  section {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    margin-bottom: 10px;
  }
  h3 :global(svg) {
    color: var(--accent);
  }
  ol {
    margin: 0;
    padding-left: 18px;
    color: var(--text-2);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    text-align: left;
    border: none;
    background: none;
    padding: 7px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    color: var(--text);
  }
  .item:hover {
    background: var(--panel-2);
  }
  .item span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
