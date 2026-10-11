# Cymatrox Viewer

Interactive cymatics laboratory written in Rust. Visualizes three wave phenomena powered by the GPU-accelerated [cymatrox](https://github.com/YaogoGerard/cymatrox) simulation crate and rendered in real-time with [Bevy](https://bevyengine.org/).

## Features

- **Three simulation modes**: Granular (Chladni-like patterns), Fluid (Faraday waves on a circular dish), Acoustic (standing pressure field with transducers).
- **Real-time rendering**: a physically-lit 3D Chladni plate with sand grains, a sparse voxel pressure field, and a top-down *cymascope* view of the fluid dish — orthographic, with a graduated instrument bezel and a Faraday mandala shaded as the reflection of a coaxial light ring (the way a real CymaScope photographs a cymaglyph). Every module runs behind a shared cinematic stack (HDR rendering, filmic Tony McMapface tonemapping and bloom).
- **Interactive control panel**: egui UI to tweak solver/liquid parameters, frequency and driving amplitude.
- **Voice control**: optional microphone FFT input to drive the frequency (pitch).
- **Frame-rate independent pacing**: simulation runs at fixed physics steps per second regardless of render FPS.
- **Production-ready packaging**: deb/AppImage (Linux), NSIS installer (Windows), DMG (macOS, unsigned). Auto-update via `cargo-packager-updater` using GitHub Releases.
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

Packaging is configured via [`Packager.toml`](./Packager.toml) and [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager). On tagged releases (`v*.*.*`), the GitHub Actions workflow builds for Linux (deb/AppImage), Windows (NSIS), and macOS (x86_64 + aarch64, DMG unsigned).

Auto-update uses `cargo-packager-updater` pointing to `https://github.com/YaogoGerard/cymatrox-viewer/releases/latest/download/latest.json`. On Windows/macOS/Linux (AppImage), the updater can download and install updates automatically; `.deb` packages are installed via the system package manager.

## macOS note

The DMG builds are **unsigned**. On first launch, right-click the app and choose "Open" to bypass Gatekeeper.

## Logging

Override default filters with the `RUST_LOG` environment variable (same syntax as `env_logger`/`tracing_subscriber`).

```bash
RUST_LOG=warn,cymatrox_viewer=info cargo run --release
```

## License

MIT OR Apache-2.0
