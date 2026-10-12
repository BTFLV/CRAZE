# Building

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs). The workspace uses edition 2024, so
  Rust 1.85 or newer is required. Note that rustup installs into `~/.cargo/bin`, which may
  not be on your `PATH` in every shell.
- **Node.js** 20 or newer with npm (CI uses Node 22).
- Platform tools for Tauri 2, see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/):
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Windows: Microsoft C++ Build Tools ("Desktop development with C++") and the WebView2
    runtime (preinstalled on Windows 10/11).
  - Linux (not an official target, but used by CI for tests): `libwebkit2gtk-4.1-dev`,
    `build-essential`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`,
    `librsvg2-dev`.

## Workspace

```text
Cargo.toml                 workspace, shared version/license, release profile
crates/craze-core          simulation library
crates/craze-cli           command line tool (binary "craze")
crates/craze-devbridge     HTTP bridge for browser-based UI tests (dev only)
app/                       Tauri app: package.json, Vite, Svelte frontend in app/src
app/src-tauri              Tauri crate "craze-app", tauri.conf.json, icons, capabilities
examples/                  example layouts, bundled with the app
```

All crates share one `target/` directory at the repository root, including the Tauri app.
Bundles therefore end up in `target/release/bundle/…`, not in `app/src-tauri/target`.

## The `app/dist` pitfall

`tauri::generate_context!` checks at compile time that `frontendDist` (`app/dist`)
exists. Because `craze-app` and `craze-devbridge` are workspace members, a plain
`cargo build --workspace` or `cargo test --workspace` fails on a fresh checkout until the
frontend has been built once:

```sh
cd app
npm ci
npm run build          # creates app/dist
cd ..
cargo test --workspace
```

If you only work on the engine, skip the app crates instead:

```sh
cargo test -p craze-core -p craze-cli
```

## Desktop app

```sh
cd app
npm install
npx tauri dev          # starts Vite on port 1420 and the app with hot reload
npx tauri build        # release build and installers for the current platform
```

`tauri dev` and `tauri build` run `npm run dev` / `npm run build` themselves
(`beforeDevCommand` / `beforeBuildCommand` in `tauri.conf.json`). The first Rust build
compiles all dependencies and takes several minutes.

Release bundles:

| Platform | Output |
| --- | --- |
| macOS | `target/release/bundle/macos/CRAZE.app`, `target/release/bundle/dmg/CRAZE_<version>_<arch>.dmg` |
| Windows | `target/release/bundle/nsis/CRAZE_<version>_x64-setup.exe`, `target/release/bundle/msi/CRAZE_<version>_x64_en-US.msi` |

`bundle.resources` copies `examples/` into the app, where `list_examples` finds them.
During development the examples are read from the repository instead.

### macOS: universal binary

A plain `npx tauri build` on a Mac produces a bundle for the host architecture only (arm64
on Apple Silicon). Release builds are universal, so one DMG runs on Apple Silicon and on
Intel Macs:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cd app
npx tauri build --target universal-apple-darwin
# → target/universal-apple-darwin/release/bundle/{macos,dmg}
```

Tauri builds both architectures and merges them with `lipo`. This doubles the Rust
compile time and the binary size compared to a single-architecture build, but keeps the
release to one macOS download and one CI job. If that ever becomes a problem, the CI matrix
can build `aarch64-apple-darwin` and `x86_64-apple-darwin` separately instead.

### Windows

Build Windows bundles on Windows. Cross-compiling from macOS does not work because the
resource compiler step (`tauri-winres`) needs `llvm-rc` and the MSVC toolchain. In CI the
Windows bundle is built on a `windows-latest` runner.

## Command line tool

```sh
cargo build --release -p craze-cli        # → target/release/craze
cargo install --path crates/craze-cli     # installs "craze" into ~/.cargo/bin
```

The CLI does not depend on the frontend, so it builds without `app/dist`.

## Build profiles

- `release`: `opt-level = 3`, thin LTO, one codegen unit. Used for bundles and benchmarks.
- `dev`: the workspace crates use `opt-level = 1`, all dependencies `opt-level = 3`, so
  debug builds still solve large problems quickly (`faer`, `rayon` and `i_overlay` are
  optimized).

## Versions and metadata

The version appears in three places and must be kept in sync: `[workspace.package]` in
`Cargo.toml`, `app/package.json` and `app/src-tauri/tauri.conf.json`. The license (MIT) is
set in the workspace and inherited by all crates.

The bundle identifier is `de.craze.app`. It also names the preferences directory, so
changing it later makes existing installations lose their saved preferences.
