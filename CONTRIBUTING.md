# Contributing

Thanks for your interest in Cymatrox Viewer. This document covers how to build,
test and change the project.

## Prerequisites

- A recent stable Rust toolchain with `rustfmt` and `clippy`:
  ```bash
  rustup component add rustfmt clippy
  ```
- On Linux, the system libraries Bevy and CPAL need (the same set CI installs):
  ```bash
  sudo apt-get install -y pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
  ```

## Build and run

```bash
cargo build              # debug
cargo run                # debug, run
cargo build --release    # release
cargo run --release
```

The release profile uses `opt-level = 3` and thin LTO across the whole Bevy
dependency tree, so a full release build is slow (several minutes). While
iterating, you can skip LTO:

```bash
CARGO_PROFILE_RELEASE_LTO=false cargo build --release
```

## Checks

Run these before pushing; CI enforces them on Linux, Windows and macOS:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

There is also a hardware-free GPU smoke test that verifies `cymatrox` and Bevy
can each own a wgpu device in the same process:

```bash
cargo run --bin gpu_smoke
```

## Project layout

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Crate root: shared state (`UiState`, `ActiveModule`, `SimState`), frequency bands, panel layout, and the Bevy app with its systems and wiring |
| `src/sim/` | The simulation thread — param sanitising, fixed-step pacing, channels (`mod.rs`), plus `granular.rs` and `fluid.rs` |
| `src/render/` | Cameras and the renderers: `granular3d.rs`, `fluid3d.rs` and `glyph.rs` (the analytic CymaScope figure) |
| `src/ui/` | egui control panel (`panel.rs`), theme, widgets and embedded fonts |
| `src/audio/` | Optional CPAL voice capture and FFT frequency detection |
| `src/bin/gpu_smoke.rs` | Dual-GPU-context smoke test |
| `docs/` | Architecture documentation and decision records, read in order from [`docs/overview.md`](./docs/overview.md) |
| `docs/design/` | Historical interface mock-ups and their captions ([`mockups.md`](./docs/design/mockups.md)) |

## How it works

- **Two GPU contexts.** The Bevy renderer runs on the main thread; the
  simulation runs on its own thread with a `cymatrox` `GpuContext`, i.e. a
  separate wgpu device. Keep them separate — the smoke test guards this.
- **Latest-wins parameters.** The UI writes `SimParams` into a shared slot; the
  sim thread reads the newest value each step, so dragging a slider never
  queues up stale work.
- **Fixed-step pacing.** Physics advances at a fixed number of steps per
  wall-clock second, decoupled from the render frame rate, with a backlog cap
  so a slow device self-limits instead of spiralling.
- **Debounced rebuilds.** Solver parameters must stay stable for a moment
  before the affected simulation is rebuilt, so dragging a slider does not
  rebuild every frame.
- **Viewer-side fluid figure.** The published `cymatrox` crate drives the
  granular module. The Fluid module's Faraday figure is computed analytically
  in the viewer (`src/render/glyph.rs`) because the crate exposes no batched
  `step_n`, and a per-step GPU readback would stall the display. Do not pull in
  upstream changes or fork the crate to work around this.

## Conventions

- **Stay viewer-side.** Do not modify the `cymatrox` crate from this repository.
- **English UI strings.** Everything the user sees is English.
- **Explain *why*.** Comments and module docs in this codebase justify design
  decisions (why a normalisation, why a cache), not the mechanics of the next
  line. Match that when you add code.
- **fmt + clippy clean.** CI treats clippy warnings as errors.
- **Tests live next to the code.** Pure logic is unit-tested in place, in a
  `#[cfg(test)] mod tests`. Test names read as sentences.
- Add a test when you fix a subtle behaviour — several invariants (slope
  scaling, motion bounds, param sanitising) are pinned by tests.

## Architecture decision records

Design decisions that are expensive to reverse are recorded as ADRs in
[`docs/decisions/`](./docs/decisions/), one file per decision, all with the same
shape: **Context, Decision, Consequences**. Write one *when the decision is
made*, and link to it from the architecture page it affects.

> The code cites `(ADR-0009)` for the crate's granular mode selection. That
> number refers to a decision documented **upstream** (in the `cymatrox`
> project), not to the viewer's own records, which are numbered from `0001`.

## Submitting changes

1. Branch off `main`.
2. Keep commits small and focused, with an imperative subject line
   (e.g. `Fluid: shade the pool as a reflected light ring`).
3. Make sure `fmt`, `clippy` and `test` pass locally.
4. Open a pull request describing the behaviour change and how you verified it.

## Logging

Configure the tracing filter with `RUST_LOG`:

```bash
RUST_LOG=warn,cymatrox_viewer=info cargo run --release
```
