<script lang="ts">
  import { X } from "@lucide/svelte";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (store.showHelp = false)} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={() => (store.showHelp = false)}>
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>{t("help.title")}</h2>
      <button class="btn ghost icon" onclick={() => (store.showHelp = false)} aria-label={t("common.close")}><X size={18} /></button>
    </header>
    <div class="body">
      {#if store.lang === "de"}
        <h3>Ablauf</h3>
        <ol>
          <li><b>Layout</b> – GDSII-Datei öffnen (Button, Drag &amp; Drop oder Beispiel). Binäres GDSII und GDS-Text aus KLayout funktionieren, auch mit Zellhierarchie.</li>
          <li><b>Layer</b> – Jedem Layer eine Rolle geben: <i>Leiter</i> (die Widerstandsschicht), <i>Terminal</i> (ideale Kontaktfläche) oder <i>Simulationsbereich</i>. Der Widerstand wird zwischen <b>Plus</b> und <b>Minus</b> berechnet. Mit „Zuordnung merken“ wird die Zuordnung für Layouts mit denselben Layern automatisch übernommen.</li>
          <li><b>Material</b> – Schichtstapel (z. B. 100 nm Platin auf 10 nm Tantal) oder einen gemessenen Flächenwiderstand eingeben. Die Betriebstemperatur wirkt über den Temperaturkoeffizienten.</li>
          <li><b>Simulation</b> – Auflösung wählen und starten. Die schmalste Leiterbahn sollte mindestens 10 Pixel breit sein.</li>
          <li><b>Ergebnisse</b> – Widerstand, Strom- und Leistungsdichte, Hotspots, Querschnitte und R(T). Mit der Auflösungsstudie lässt sich die Genauigkeit abschätzen.</li>
          <li><b>Optimierung</b> – Pfadbreiten automatisch so anpassen, dass ein Zielwiderstand (z. B. 1000 Ω für Pt1000) erreicht wird, und das neue Layout speichern.</li>
        </ol>
        <h3>Layout in KLayout vorbereiten</h3>
        <ul>
          <li>Leiterbahnen am besten als <b>Pfad</b> mit Breite zeichnen – dann kann die Optimierung die Breite anpassen. Polygone funktionieren für die Simulation genauso.</li>
          <li>Die Kontaktflächen (Pads) auf eigene Layer legen. Sie dürfen die Leiterbahn überlappen.</li>
          <li>Speichern als „GDS2“ (binär) – das ist das Standardformat in KLayout.</li>
          <li>Klassische CRAZE-Belegung (wird automatisch erkannt): Layer 1 = Bereich, 2 = Leiter, 3 = Plus, 4 = Minus.</li>
        </ul>
        <h3>Was bedeutet „Quadrate“?</h3>
        <p>Der Widerstand einer dünnen Schicht ist R = N · R□. R□ (Flächenwiderstand) hängt nur von Material und Dicke ab, N (Anzahl Quadrate) nur von der Geometrie. Ein Streifen der Länge L und Breite W hat N = L / W. Ecken, Pads und Stromverdrängung berücksichtigt die Simulation automatisch.</p>
        <h3>Wie rechnet CRAZE?</h3>
        <p>Das Layout wird exakt flächentreu in Pixel zerlegt. Teilweise bedeckte Randpixel werden mit einer geraden Materialkante innerhalb des Pixels modelliert. So entsteht ein Widerstandsnetz (Finite-Volumen-Verfahren), dessen Gleichungssystem direkt per Cholesky-Zerlegung oder iterativ (CG mit unvollständiger Cholesky-Vorkonditionierung) gelöst wird.</p>
        <h3>Bedienung</h3>
        <ul>
          <li>Mausrad / Pinch: Zoomen · Ziehen: Verschieben · Doppelklick oder <kbd>F</kbd>: Einpassen</li>
          <li>Lineal-Werkzeug: Linie für den Querschnitt ziehen · <kbd>Esc</kbd>: zurück zum Verschieben</li>
          <li><kbd>Strg/⌘ + O</kbd> öffnen · <kbd>Strg/⌘ + S</kbd> speichern · <kbd>Strg/⌘ + Enter</kbd> simulieren</li>
        </ul>
      {:else}
        <h3>Workflow</h3>
        <ol>
          <li><b>Layout</b> – open a GDSII file (button, drag &amp; drop or example). Binary GDSII and KLayout GDS text work, including cell hierarchy.</li>
          <li><b>Layers</b> – give every layer a role: <i>conductor</i> (resistive film), <i>terminal</i> (ideal contact) or <i>simulation area</i>. The resistance is computed between <b>plus</b> and <b>minus</b>.</li>
          <li><b>Material</b> – enter a layer stack or a measured sheet resistance and the operating temperature.</li>
          <li><b>Simulation</b> – choose the resolution (narrowest trace ≥ 10 pixels) and start.</li>
          <li><b>Results</b> – resistance, current and power density, hotspots, cross-sections and R(T).</li>
          <li><b>Optimization</b> – scale path widths to reach a target resistance and save the new layout.</li>
        </ol>
        <h3>Squares</h3>
        <p>R = N · R□. R□ depends on material and thickness only, N (number of squares) on geometry only. A strip of length L and width W has N = L / W.</p>
        <h3>Method</h3>
        <p>The layout is rasterized with exact area coverage; partially covered pixels are modeled with a straight sub-pixel material edge. The resulting finite-volume resistor network is solved with a sparse Cholesky factorization or with IC(0)-preconditioned conjugate gradients.</p>
        <h3>Controls</h3>
        <ul>
          <li>Wheel / pinch: zoom · drag: pan · double click or <kbd>F</kbd>: fit</li>
          <li>Ruler tool: drag a cross-section line · <kbd>Esc</kbd>: back to pan</li>
          <li><kbd>Ctrl/⌘ + O</kbd> open · <kbd>Ctrl/⌘ + S</kbd> save · <kbd>Ctrl/⌘ + Enter</kbd> simulate</li>
        </ul>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(10, 12, 18, 0.45);
    display: grid;
    place-items: center;
    z-index: 50;
    backdrop-filter: blur(2px);
  }
  .modal {
    width: min(760px, calc(100vw - 48px));
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-radius: 14px;
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    user-select: text;
    -webkit-user-select: text;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }
  .body {
    overflow-y: auto;
    padding: 6px 24px 22px;
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--text-2);
  }
  h3 {
    margin: 18px 0 6px;
    font-size: 14px;
    color: var(--text);
  }
  ol,
  ul {
    padding-left: 20px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  kbd {
    font-family: var(--mono);
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid var(--border-strong);
    background: var(--panel-2);
  }
</style>
