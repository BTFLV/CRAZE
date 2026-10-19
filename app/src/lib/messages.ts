// Messages from the Rust backend are English. This table translates them for the German UI.
// Keep it in sync with the messages in craze-core and app/src-tauri. Unknown messages are
// shown unchanged.
import { store } from "./store.svelte";

/** Error text of a cancelled job, as produced by the backend. */
export const CANCELLED = "Cancelled";

type Rule = [RegExp, string];

const DE: Rule[] = [
  // Progress
  [/^Rasterizing layout …$/, "Layout wird gerastert …"],
  [/^Building resistor network …$/, "Widerstandsnetz wird aufgebaut …"],
  [/^Solving (\d+) unknowns directly …$/, "Gleichungssystem mit $1 Unbekannten wird direkt gelöst …"],
  [/^Solving (\d+) unknowns iteratively …$/, "Gleichungssystem mit $1 Unbekannten wird iterativ gelöst …"],
  [/^Iteration (\d+), residual (.+)$/, "Iteration $1, Residuum $2"],
  [/^Computing results …$/, "Ergebnisse werden berechnet …"],
  // Warnings
  [
    /^The narrowest trace is only (.+) pixels wide\. Use a resolution of at least (\d+) pixels for accurate results\.$/,
    "Die schmalste Leiterbahn ist nur $1 Pixel breit. Für genaue Ergebnisse mindestens $2 Pixel Auflösung verwenden.",
  ],
  [
    /^(\d+) pixels \((.+) %\) of conductor have no connection to a terminal and are ignored\.$/,
    "$1 Pixel ($2 %) Leitermaterial haben keine Verbindung zu einem Terminal und werden ignoriert.",
  ],
  // Setup and simulation errors
  [/^Cancelled$/, "Abgebrochen"],
  [/^No layer is assigned as conductor \(resistive film\)\.$/, "Es ist kein Layer als Leiter (Widerstandsschicht) zugeordnet."],
  [/^At least two terminals are required\.$/, "Es werden mindestens zwei Terminals benötigt."],
  [/^Plus and minus terminal must be different\.$/, "Plus- und Minus-Terminal müssen verschieden sein."],
  [/^Terminal “(.*)” has no geometry\. Please assign a layer\.$/, "Terminal „$1“ hat keine Geometrie – bitte einen Layer zuordnen."],
  [/^Plus or minus terminal is invalid\.$/, "Plus- oder Minus-Terminal ist ungültig."],
  [/^Plus or minus terminal is missing\.$/, "Plus- oder Minus-Terminal fehlt."],
  [
    /^The terminals “(.*)” and “(.*)” touch each other \(short circuit\)\. Please separate the contact pads\.$/,
    "Die Terminals „$1“ und „$2“ berühren sich (Kurzschluss). Bitte die Kontaktflächen trennen.",
  ],
  [/^No material was found on the conductor layers\.$/, "Auf den Leiter-Layern wurde kein Material gefunden."],
  [
    /^There is no conducting path between “(.*)” and “(.*)”\. Do both terminals touch the trace\?$/,
    "Zwischen „$1“ und „$2“ besteht keine leitende Verbindung. Berühren beide Terminals die Leiterbahn?",
  ],
  [/^No geometry found on conductor or terminal layers\.$/, "Keine Geometrie auf Leiter- oder Terminal-Layern gefunden."],
  [/^The resolution is too high \(more than 60 million pixels\)\.$/, "Auflösung ist zu hoch (mehr als 60 Mio. Pixel)."],
  [
    /^The optimization changes path widths, but the conductor layers contain no paths \(only polygons\)\.$/,
    "Die Optimierung verändert Pfadbreiten, aber auf den Leiter-Layern gibt es keine Pfade (nur Polygone).",
  ],
  [/^Target resistance and sheet resistance must be positive\.$/, "Zielwiderstand und Flächenwiderstand müssen positiv sein."],
  [/^Solver error: No current flows\. Is the trace interrupted\?$/, "Solver-Fehler: Es fließt kein Strom – ist die Leiterbahn unterbrochen?"],
  [
    /^Solver error: No convergence after (\d+) iterations \(relative residual (.+)\)\. Increase the iterations or use the direct solver\.$/,
    "Solver-Fehler: Keine Konvergenz nach $1 Iterationen (rel. Residuum $2). Iterationen erhöhen oder direkten Löser verwenden.",
  ],
  // Backend
  [/^The file contains no cells\.$/, "Die Datei enthält keine Zellen."],
  [/^No layout is loaded\.$/, "Es ist kein Layout geladen."],
  [/^A computation is already running\.$/, "Es läuft bereits eine Berechnung."],
  [/^The cell contains no geometry\.$/, "Die Zelle enthält keine Geometrie."],
  [/^No results available\.$/, "Keine Ergebnisse vorhanden."],
  [/^Cannot read the file: /, "Datei kann nicht gelesen werden: "],
  [/^Saving failed: /, "Speichern fehlgeschlagen: "],
  // Prefixes of technical errors; the detail stays English.
  [/^GDS error: /, "GDS-Fehler: "],
  [/^Solver error: /, "Solver-Fehler: "],
  [/^File error: /, "Dateifehler: "],
  [/^Format error: /, "Formatfehler: "],
];

function translateLine(line: string): string {
  for (const [re, de] of DE) if (re.test(line)) return line.replace(re, de);
  return line;
}

/** Translates a backend message (possibly several lines) into the UI language. */
export function localize(message: string): string {
  if (store.lang !== "de") return message;
  return message.split("\n").map(translateLine).join("\n");
}

/** Text of an error thrown by a backend command, in the UI language. */
export function errorText(e: unknown): string {
  return localize(String(e));
}
