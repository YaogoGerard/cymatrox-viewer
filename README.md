# Cymatrox Viewer

<div align="center">

[![CI](https://github.com/YaogoGerard/cymatrox-viewer/actions/workflows/ci.yml/badge.svg)](https://github.com/YaogoGerard/cymatrox-viewer/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://github.com/YaogoGerard/cymatrox-viewer#license) [![Rust](https://img.shields.io/badge/rust-stable-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/) [![Bevy](https://img.shields.io/badge/Bevy-0.19-blueviolet)](https://bevyengine.org/) [![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey)](https://github.com/YaogoGerard/cymatrox-viewer#packaging--releases)

</div>

Interactive cymatics laboratory written in Rust. Visualizes two wave phenomena — granular (Chladni patterns) and fluid (Faraday waves) — with the sand driven by the GPU-accelerated [cymatrox](https://github.com/YaogoGerard/cymatrox) simulation crate and both modules rendered in real-time with [Bevy](https://bevyengine.org/).

## Features

- **Two simulation modes**: Granular (Chladni-like patterns), Fluid (Faraday waves on a circular dish).
- **Real-time rendering**: a physically-lit 3D Chladni plate with sand grains and a top-down *cymascope* view of the fluid dish — orthographic, with a graduated instrument bezel and a Faraday mandala shaded as the reflection of a coaxial light ring (the way a real CymaScope photographs a cymaglyph). While a tone is held the surface keeps moving — travelling capillary ripples and a slow slosh — and a tone change grows the new figure in over about a second. Every module runs behind a shared cinematic stack (HDR rendering, filmic Tony McMapface tonemapping and bloom).
- **Interactive control panel**: egui UI to tweak solver/liquid parameters, frequency and driving amplitude.
- **Voice control**: optional microphone FFT input to drive the frequency (pitch).
- **Frame-rate independent pacing**: simulation runs at fixed physics steps per second regardless of render FPS.
- **Production-ready packaging**: deb/AppImage (Linux), NSIS installer (Windows), DMG (macOS, unsigned).
- **Traced logging**: configurable with `RUST_LOG` (e.g. `RUST_LOG=info,cymatrox_viewer=debug`).

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run
cargo run --release
```

## GPU dual-context smoke test

Validates that cymatrox and Bevy can both initialize their own wgpu devices in the same process (step 4b from the plan).

```bash
cargo run --bin gpu_smoke
```

## Tests

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Packaging & Releases

Packaging is configured through the `[package.metadata.packager]` table in [`Cargo.toml`](./Cargo.toml) and [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager). On tagged releases (`v*.*.*`), the GitHub Actions workflow builds each target with `cargo build --release --target …` and writes the installers to `dist/`: **deb + AppImage** (Linux), **NSIS** (Windows), and **DMG** (macOS x86_64 + aarch64, unsigned).

Auto-update is not wired up yet: producing `cargo-packager-updater` artifacts needs a signing key (`CARGO_PACKAGER_SIGN_PRIVATE_KEY`) and updater metadata, which are not configured.

## macOS note

The DMG builds are **unsigned**. On first launch, right-click the app and choose "Open" to bypass Gatekeeper.

## Logging

Override default filters with the `RUST_LOG` environment variable (same syntax as `env_logger`/`tracing_subscriber`).

```bash
RUST_LOG=warn,cymatrox_viewer=info cargo run --release
```

## Documentation

Architecture and design notes live in [`docs/`](./docs/overview.md). They are
meant to be read in order — **Overview → Architecture → Runtime → Data flow →
Simulation → Rendering → UI → Decision records → Glossary** — starting at
[`docs/overview.md`](./docs/overview.md); each page links to the next.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for prerequisites, the check commands CI enforces, and the project layout.

## License

MIT — see [LICENSE](./LICENSE).
