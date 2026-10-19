// Self-contained HTML report of a simulation.
import { store } from "./store.svelte";
import { duration, length, num, sci, si, sig } from "./format";
import { sheetResistance, volumeCurrentFactor, type Electrical } from "./physics";
import { keyStr } from "./types";
import { materialName } from "./i18n";
import { localize } from "./messages";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export function buildReport(
  image: string,
  el: Electrical,
  peaks: { K: number; J: number | null; p: number } | null,
  extrap: { value: number; err: number } | null,
): string {
  const s = store.summary!;
  const l = store.layout!;
  const setup = store.setup!;
  const de = store.lang === "de";
  const T = (a: string, b: string) => (de ? a : b);
  const roleName = (r: (typeof setup.layers)[number]["role"]) =>
    r.type === "conductor"
      ? T("Leiter", "Conductor")
      : r.type === "terminal"
        ? `Terminal ${esc(setup.terminals[r.id]?.name ?? "")}`
        : r.type === "region"
          ? T("Simulationsbereich", "Simulation area")
          : T("ignoriert", "ignored");
  const stackRows =
    store.stack.mode === "layers"
      ? store.stack.layers
          .map(
            (x) =>
              `<tr><td>${esc(materialName(x.material.name))}</td><td>${num(x.thicknessNm, 4)} nm</td><td>${sci(x.material.resistivity, 3)} Ω·m</td><td>${sig(x.material.tcr * 1e6, 4)} ppm/K</td></tr>`,
          )
          .join("")
      : `<tr><td colspan="4">${T("Gemessen", "Measured")}: R□ = ${si(store.stack.sheetResistance, "Ω")} @ ${store.stack.tRefC} °C, α = ${sig(store.stack.tcr * 1e6, 4)} ppm/K</td></tr>`;
  const vol = volumeCurrentFactor(store.stack, store.temperature);
  const temps = [-40, 0, 20, 50, 100, 150, 200, 300];
  const date = new Date().toLocaleString(de ? "de-DE" : "en-US");
  const st = s.stats;

  return `<!doctype html>
<html lang="${store.lang}"><head><meta charset="utf-8"><title>CRAZE – ${esc(l.fileName)}</title>
<style>
body{font-family:-apple-system,"Segoe UI",system-ui,sans-serif;color:#171b26;max-width:900px;margin:32px auto;padding:0 24px;line-height:1.5}
h1{font-size:24px;margin:0}h2{font-size:16px;margin:28px 0 8px;border-bottom:1px solid #e1e4ea;padding-bottom:4px}
.sub{color:#6a7285}.big{font-size:34px;font-weight:700;color:#4f3fd6;margin:12px 0 0}
table{border-collapse:collapse;width:100%;font-size:13px}td,th{padding:5px 8px;border-bottom:1px solid #e9ebf0;text-align:left}
th{color:#6a7285;font-weight:600}td.r{text-align:right;font-variant-numeric:tabular-nums}
img{max-width:100%;border:1px solid #e1e4ea;border-radius:8px;margin-top:8px}
@media print{body{margin:0}}
</style></head><body>
<h1>CRAZE – ${T("Simulationsbericht", "Simulation report")}</h1>
<div class="sub">${esc(l.fileName)} · ${T("Zelle", "Cell")} ${esc(l.topCell)} · ${date}</div>
<div class="big">R = ${si(el.resistance, "Ω", 5)}</div>
<div class="sub">${sig(s.squares, 6)} ${T("Quadrate", "squares")} × R□ ${si(store.sheetResistance, "Ω")} ${T("bei", "at")} ${store.temperature} °C</div>
<img src="${image}" alt="">
<h2>${T("Betriebspunkt", "Operating point")}</h2>
<table>
<tr><td>${T("Spannung", "Voltage")}</td><td class="r">${si(el.voltage, "V")}</td></tr>
<tr><td>${T("Strom", "Current")}</td><td class="r">${si(el.current, "A")}</td></tr>
<tr><td>${T("Leistung", "Power")}</td><td class="r">${si(el.power, "W")}</td></tr>
${peaks ? `${peaks.J != null && vol ? `<tr><td>${T("Max. Stromdichte", "Max. current density")} (${esc(materialName(vol.layer))})</td><td class="r">${sig(peaks.J / 1e6, 4)} A/mm²</td></tr>` : ""}
<tr><td>${T("Max. Flächenstromdichte", "Max. sheet current density")}</td><td class="r">${si(peaks.K, "A/m")}</td></tr>
<tr><td>${T("Max. Leistungsdichte", "Max. power density")}</td><td class="r">${si(peaks.p / 1e6, "W/mm²")}</td></tr>` : ""}
${setup.terminals.map((tm, i) => `<tr><td>${T("Potential", "Potential")} ${esc(tm.name)}</td><td class="r">${si(s.terminalPotentials[i] * el.voltage, "V")}</td></tr>`).join("")}
</table>
<h2>${T("Temperaturabhängigkeit", "Temperature dependence")}</h2>
<table><tr><th>T</th>${temps.map((x) => `<th>${x} °C</th>`).join("")}</tr>
<tr><td>R</td>${temps.map((x) => `<td class="r">${si(s.squares * sheetResistance(store.stack, x), "Ω", 5)}</td>`).join("")}</tr></table>
<h2>${T("Schichtstapel", "Layer stack")}</h2>
<table><tr><th>${T("Material", "Material")}</th><th>${T("Dicke", "Thickness")}</th><th>ρ (20 °C)</th><th>α</th></tr>${stackRows}</table>
<h2>${T("Layer-Zuordnung", "Layer assignment")}</h2>
<table>${setup.layers.filter((a) => a.role.type !== "ignore").map((a) => `<tr><td>${keyStr(a.key)}</td><td>${roleName(a.role)}</td></tr>`).join("")}
<tr><td>Plus / Minus</td><td>${esc(setup.terminals[setup.source]?.name ?? "")} / ${esc(setup.terminals[setup.sink]?.name ?? "")}</td></tr></table>
<h2>${T("Numerik", "Numerics")}</h2>
<table>
<tr><td>${T("Raster", "Grid")}</td><td class="r">${s.grid.nx} × ${s.grid.ny} px, ${length(s.grid.pxUm)} / px</td></tr>
<tr><td>${T("Unbekannte", "Unknowns")}</td><td class="r">${num(st.unknowns, 0)}</td></tr>
<tr><td>${T("Löser", "Solver")}</td><td class="r">${st.solver === "direct" ? "Cholesky" : `PCG/IC(0), ${st.iterations} it.`}</td></tr>
<tr><td>${T("Relatives Residuum", "Relative residual")}</td><td class="r">${sci(st.relativeResidual, 1)}</td></tr>
<tr><td>${T("Strombilanz-Fehler", "Current balance error")}</td><td class="r">${sci(st.currentBalanceError, 1)}</td></tr>
<tr><td>${T("Rechenzeit", "Computation time")}</td><td class="r">${duration(st.timeRasterMs + st.timeBuildMs + st.timeSolveMs + st.timePostMs)}</td></tr>
${extrap ? `<tr><td>${T("Extrapoliert (∞ Pixel)", "Extrapolated (∞ pixels)")}</td><td class="r">${si(extrap.value * store.sheetResistance, "Ω", 6)} (${T("Fehler", "error")} ≈ ${sig(extrap.err * 100, 2)} %)</td></tr>` : ""}
</table>
${s.warnings.length ? `<h2>${T("Hinweise", "Notes")}</h2><ul>${s.warnings.map((w) => `<li>${esc(localize(w))}</li>`).join("")}</ul>` : ""}
<p class="sub" style="margin-top:32px">CRAZE ${__APP_VERSION__}</p>
</body></html>`;
}
