# Testing

## Automated tests

```sh
cd app && npm ci && npm run build && cd ..   # app/dist is needed for the workspace build
cargo test --workspace                        # unit and validation tests
cd app && npm run check                       # svelte-check (TypeScript + Svelte)
```

CI runs exactly these steps on every push and pull request (see
[releasing.md](releasing.md)).

### What is covered

| Location | Tests |
| --- | --- |
| `craze-core/src/geometry.rs` | Path expansion (flush/square ends, round caps), STRANS rotation and reflection. |
| `craze-core/src/raster.rs` | Exact triangle area, partial pixel values, holes, merging of overlapping shapes with opposite orientation. |
| `craze-core/src/solver.rs` | Direct and PCG solver against the analytic 1D Laplacian. |
| `craze-core/src/materials.rs` | Parallel layer stack and temperature dependence. |
| `craze-core/tests/validation.rs` | KLayout reference areas, binary/text equivalence, binary round trip, and the analytic physics cases (bars, Corbino ring, 90° corner, floating terminal, iterative vs. direct). The table with results is in [numerics.md](numerics.md#11-validation). |

Test layouts live in `crates/craze-core/tests/data`:

- `hier.gds` / `hier.txt`: hierarchical test cell (binary and KLayout text) with SREF,
  AREF, rotation, mirroring, magnification and all path types. `hier_ref.json` holds the
  merged area per layer as computed by KLayout.
- `2_paths.gds`, `8_paths.gds`, `G1_R4.gds`, `crowd_4_1.gds`: binary versions of the
  examples, compared with the `.gds2` text files in `examples/`.

Several validation tests print their numbers. To see them:

```sh
cargo test --release -p craze-core --test validation -- --nocapture
```

### Adding a validation case

Prefer cases with an analytic or independently computed reference. Build the layout in
code with the `Builder` helper in `validation.rs` (or add a small GDS file to
`tests/data`), simulate with `squares(…)`, and assert a relative tolerance that the case
meets with some margin at a moderate resolution. Keep resolutions low enough that the whole
suite stays fast in debug builds.

## Command line checks

The CLI is the quickest way to check results on real layouts:

```sh
cargo build --release -p craze-cli
target/release/craze sweep examples/Pt1000_meander.gds --resolutions 500,1000,2000,4000
```

## UI tests in a browser

The frontend can run in an ordinary browser against the real backend through
`craze-devbridge` (see [architecture.md](architecture.md#development-bridge)). This is used
for automated UI tests and for the README screenshots, and it works where desktop screen
capture is not available.

```sh
cargo build --release -p craze-devbridge
target/release/craze-devbridge                 # listens on 127.0.0.1:1421
cd app && npx vite --port 1420                 # second terminal
```

Open http://localhost:1420. Without Tauri, `main.ts` installs a `mockIPC` handler that
forwards every command to the bridge. Things to know when writing tests:

- Native file dialogs return `window.__crazeDialogPath`. Set it before clicking *Open*
  or *Save*. Examples can be opened from the welcome screen without a dialog.
- Progress channels are not forwarded, so progress bars stay empty. Wait for the result
  instead (for example for `.result-card` after starting a simulation).
- Preferences are kept in memory by the bridge. Restart it for a clean state; a fresh
  bridge starts with the German UI (the language button has the title "Sprache").
- PNG export (`write_base64_file`) does nothing.
- `CRAZE_TEST_FILE=/path/to/layout.gds` makes the app open that file on start.
- `CRAZE_BRIDGE_PORT` changes the port (the frontend expects 1421 by default).

> The bridge answers requests from any origin and can read and write any file the user
> can. Run it only while testing, on a trusted machine, and stop it afterwards.

Use WebKit when you drive the UI with Playwright. It is the engine of the macOS webview,
so rendering and focus behaviour match the app (for example, clicking a button does not
move keyboard focus in WebKit).

## Regenerating the screenshots

`scripts/screenshots.mjs` drives the steps above with Playwright and WebKit at 1440×900 and
writes the README images to `docs/images`. Playwright is deliberately not a project
dependency; install it anywhere:

```sh
npm install --prefix /tmp/craze-shots playwright@1.63.0 sharp
npx --prefix /tmp/craze-shots playwright install webkit   # only if WebKit is not cached yet

# with the bridge and Vite running as above:
SCREENSHOT_TOOLS=/tmp/craze-shots node scripts/screenshots.mjs
```

The script restarts nothing itself, so start from a freshly launched bridge. It switches
the UI to English, opens the bundled examples, runs a simulation, the optimization and the
four-terminal example, and saves 11 images. The shots use the dark appearance (the browser
reports a dark color scheme); one shot switches the app to light. With `sharp` installed
the PNGs are palette-quantized (about 30–80 KB each). Look at every image before
committing it.
