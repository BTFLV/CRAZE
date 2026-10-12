// Regenerates the README screenshots in docs/images.
//
// Drives the frontend in WebKit (the macOS webview engine) against the development
// bridge, so no desktop screen capture is needed. See docs/testing.md for the setup:
//
//   1. cargo build --release -p craze-devbridge && target/release/craze-devbridge
//   2. cd app && npx vite --port 1420
//   3. node scripts/screenshots.mjs [output-dir]
//
// Playwright (and optionally sharp) are not project dependencies. Install them into any
// directory and pass it as SCREENSHOT_TOOLS, e.g.
//
//   npm install --prefix /tmp/craze-shots playwright@1.63.0 sharp
//   SCREENSHOT_TOOLS=/tmp/craze-shots node scripts/screenshots.mjs
//
// When sharp is available the PNGs are palette-quantized to keep the repository small.

import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

const tools = process.env.SCREENSHOT_TOOLS;
const load = (name) =>
  import(tools ? pathToFileURL(createRequire(join(resolve(tools), "noop.js")).resolve(name)).href : name).then(
    (m) => m.default ?? m,
  );

const { webkit } = await load("playwright");
const sharp = await load("sharp").catch(() => null);
if (!sharp) console.warn("sharp not found: writing unoptimized PNGs");

const URL = process.env.CRAZE_URL ?? "http://localhost:1420";
const OUT = resolve(process.argv[2] ?? "docs/images");
mkdirSync(OUT, { recursive: true });

const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: "dark", deviceScaleFactor: 1 });
page.on("dialog", (d) => d.accept());
page.on("pageerror", (e) => console.error("page error:", e.message));

const shot = async (name) => {
  await page.waitForTimeout(400);
  const path = join(OUT, `${name}.png`);
  const png = await page.screenshot();
  if (sharp) await sharp(png).png({ palette: true, quality: 90, effort: 10, compressionLevel: 9 }).toFile(path);
  else writeFileSync(path, png);
  console.log("saved", name);
};
const button = (text) => page.locator("button", { hasText: text }).first();
const step = (n) => page.locator(".stepper .step").nth(n - 1);
const canvas = () => page.locator("canvas.overlay");

async function openExample(name) {
  await button(name).click();
  await page.locator(".panel h2").first().waitFor();
}

async function simulate() {
  await page.locator("button.primary", { hasText: "Start simulation" }).click();
  await page.locator(".result-card").waitFor({ timeout: 120_000 });
  await page.waitForTimeout(300);
}

/** Drags a line on the viewer between two points given as fractions of the canvas size. */
async function dragOnViewer(a, b) {
  const box = await canvas().boundingBox();
  await page.mouse.move(box.x + a[0] * box.width, box.y + a[1] * box.height);
  await page.mouse.down();
  await page.mouse.move(box.x + b[0] * box.width, box.y + b[1] * box.height, { steps: 12 });
  await page.mouse.up();
}

await page.goto(URL);
await page.locator(".welcome").waitFor();
await page.waitForTimeout(500);

// English UI (the bridge keeps preferences in memory, so restart it for a clean state).
const lang = page.locator('button[title="Sprache"]');
if (await lang.count()) await lang.click();
await page.locator("h1", { hasText: "Welcome to CRAZE" }).waitFor();
await shot("welcome");

// --- Pt1000 meander ---------------------------------------------------------------------
await openExample("Pt1000 meander");
await shot("layers");

await step(3).click();
await shot("material");

await step(4).click();
await shot("simulation");

await simulate();
await shot("results-current");

await page.locator("label.check", { hasText: "Current direction" }).locator("input").check();
await button("Show hotspot").click();
await page.waitForTimeout(800);
await shot("hotspot");

await page.locator("label.check", { hasText: "Current direction" }).locator("input").uncheck();
await page.keyboard.press("f");
await page.locator(".panel .segmented button", { hasText: "Potential" }).click();
// inferno's dark end vanishes on the dark viewer background; turbo keeps 0 V visible.
const colormap = page.locator(".panel select", { has: page.locator('option[value="turbo"]') });
await colormap.selectOption("turbo");
await page.locator("label.check", { hasText: "Equipotential lines" }).locator("input").check();
await shot("potential");

await page.locator("label.check", { hasText: "Equipotential lines" }).locator("input").uncheck();
await colormap.selectOption("inferno");
await page.locator(".panel .segmented button", { hasText: "Current density" }).click();
await button("Draw line").click();
await dragOnViewer(...JSON.parse(process.env.CRAZE_CUT ?? "[[0.21,0.78],[0.31,0.78]]"));
await page.locator(".panel .section-title", { hasText: "Cross-section" }).scrollIntoViewIfNeeded();
await shot("cross-section");
await page.keyboard.press("Escape");

await step(6).click();
await button("Start optimization").click();
await page.locator(".panel", { hasText: "Save optimized layout" }).waitFor({ timeout: 300_000 });
await shot("optimization");

// --- Light mode (all other shots use the dark system theme) ---------------------------------
await step(5).click();
await page.keyboard.press("f");
const themeBtn = page.locator('.topbar button[title^="Appearance"]');
for (let i = 0; i < 3 && !(await page.evaluate(() => document.documentElement.dataset.theme === "light")); i++)
  await themeBtn.click();
await shot("light");
while (await page.evaluate(() => document.documentElement.dataset.theme)) await themeBtn.click();

// --- Four-terminal Kelvin structure -------------------------------------------------------------
await button("New").click();
await page.locator(".welcome").waitFor();
await openExample("Kelvin four terminal");
await button("Add terminal").click();
await button("Add terminal").click();
const names = page.locator(".terminal input");
await names.nth(2).fill("Sense+");
await names.nth(3).fill("Sense−");
for (const [key, terminal] of [["5/0", "t2"], ["6/0", "t3"]]) {
  const row = page.locator(".layer", { has: page.locator(".key", { hasText: key }) });
  await row.locator("select").selectOption(terminal);
}
await step(4).click();
await simulate();
// The viewer keeps the last hotspot focus request across layouts, so fit explicitly.
await page.locator(".viewer .toolbar .tool-btn").click();
await page.locator(".panel .section-title", { hasText: "Terminals" }).scrollIntoViewIfNeeded();
await shot("four-terminal");

await browser.close();
