# Contributing to Cymatrox Viewer

Thanks for wanting to help! This is a small project, and every contribution
counts — a bug report, a documentation fix, an idea or a patch are all welcome.

You do **not** need to be a Rust expert. If you can read the code and explain
your reasoning, you can contribute here.

## Ways to contribute

- **Report a bug.** Open an issue and say what you did, what you expected and
  what happened instead. A screenshot or the log output helps a lot.
- **Suggest an improvement.** A new visualisation, a parameter, something
  easier to use — open an issue and we will talk about it.
- **Improve the documentation.** The pages in [`docs/`](./docs/overview.md) and
  this file. Typos and confusing sentences are worth fixing.
- **Write code.** Fix a bug, add a test, improve the interface. See
  [Your first change](#your-first-change).

If you are not sure whether something is worth doing, open an issue first. A
question costs less than a pull request nobody can merge.

## New to Rust?

Welcome! A few pointers:

- Install the toolchain with [rustup](https://rustup.rs/). It gives you `cargo`
  (the build tool), `rustfmt` (formatting) and `clippy` (linting).
- [The Rust Book](https://doc.rust-lang.org/book/) is the standard
  introduction; chapters 1 to 10 cover everything used here.
- `cargo doc --open` builds the documentation of this project and of its
  dependencies, and opens it in your browser.

Nobody expects perfect code on the first try. Review comments are about the
code, never about you.

## Setup

1. **Install the extra tools CI uses**

   ```bash
   rustup component add rustfmt clippy
   ```

2. **Clone the repository**

   ```bash
   git clone https://github.com/YaogoGerard/cymatrox-viewer.git
   cd cymatrox-viewer
   ```

3. **Install the system libraries — Linux only.** Bevy and the microphone
   capture need them:

   ```bash
   sudo apt-get install -y pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
   ```

   On Windows and macOS there is nothing to install.

4. **Build and run**

   ```bash
   cargo run              # debug build, then start the app
   cargo build --release  # optimised build
   cargo run --release    # run the optimised build
   ```

   The first build compiles the whole Bevy dependency tree, so it takes a
   while; later builds are fast. The release profile uses thin LTO, which is
   slow to link — while iterating you can turn it off:

   ```bash
   CARGO_PROFILE_RELEASE_LTO=false cargo build --release
   ```

## Your first change

Work on a branch, never directly on `main`:

```bash
git checkout -b fix/fluid-camera-framing   # pick a short, descriptive name
# ... edit some files ...
cargo fmt --all                            # apply the canonical formatting
cargo clippy --all-targets -- -D warnings  # lint
cargo test --all                           # unit tests
git add -A
git commit -m "Fluid: keep the dish framed when the window is resized"
git push origin fix/fluid-camera-framing
```

Then open a pull request on GitHub (the page suggests it as soon as you push).

Good places to start, roughly from easiest to hardest:

- Fix a typo or an unclear sentence in the README or in `docs/`.
- Improve a label or a tooltip in the control panel (`src/ui/panel.rs`).
- Adjust the colours or the spacing (`src/ui/theme.rs`).
- Run the app on your machine and report anything that feels off — an issue is
  a real contribution.
- Add a unit test for a function that has none.
- Add a screenshot of a configuration you like to `docs/screenshots/`.

## Before you push: the checks

CI runs these three commands on Linux, Windows and macOS. Run them locally so
your pull request is green on the first try:

```bash
cargo fmt --all -- --check                    # formatting
cargo clippy --all-targets -- -D warnings     # lints: warnings are errors in CI
cargo test --all                              # tests
```

There is also a small GPU test that checks that `cymatrox` and Bevy can each
own their own wgpu device in the same process. It only uses the GPU and never
opens a window:

```bash
cargo run --bin gpu_smoke
```

## Project layout

| Path | What lives there |
| --- | --- |
| `src/main.rs` | Crate root: shared state (`UiState`, `ActiveModule`, `SimState`), the panel layout, and the Bevy app with its systems |
| `src/sim/` | The simulation thread: parameter sanitising, fixed-step pacing and channels (`mod.rs`), plus `granular.rs` and `fluid.rs` |
| `src/render/` | Cameras and renderers: `granular3d.rs`, `fluid3d.rs` and `glyph.rs` (the analytic CymaScope figure) |
| `src/ui/` | egui control panel (`panel.rs`), theme, widgets and embedded fonts |
| `src/audio/` | Optional microphone capture (CPAL) and FFT pitch detection |
| `src/bin/gpu_smoke.rs` | The dual-GPU-context smoke test |
| `build/icons/` | App icons used by the installers; `build.rs` embeds the Windows one |
| `docs/` | Architecture notes and decision records, read in order from [`docs/overview.md`](./docs/overview.md) |
| `docs/screenshots/` | The images shown in the README |

## How it works

Enough to find your way around:

- **Two GPU contexts.** Bevy renders on the main thread; the simulation runs on
  its own thread with its own `cymatrox` device. The smoke test above guards
  that separation — keep them separate.
- **Latest-wins parameters.** The panel writes the current parameters into a
  shared slot and the simulation thread reads the newest value each step, so
  dragging a slider never queues up stale work.
- **Fixed-step pacing.** The physics advances a fixed number of steps per
  second of wall-clock time, independently of the frame rate, with a backlog
  cap so a slow machine limits itself instead of lagging behind forever.
- **Debounced rebuilds.** Solver parameters must stay stable for a short moment
  before the simulation is rebuilt, so dragging a slider does not rebuild on
  every frame.
- **The fluid figure is computed in the viewer.** The published `cymatrox`
  crate drives the granular module; the Fluid module's Faraday figure is
  computed analytically in `src/render/glyph.rs`, because the crate exposes no
  batched step and a per-step GPU readback would stall the display.

## Conventions

- **Stay viewer-side.** Do not modify the `cymatrox` crate from this
  repository — work around a limitation here, or discuss it upstream.
- **English everywhere.** UI strings, comments and documentation are English.
- **Explain *why*.** Comments in this codebase justify design decisions (why a
  normalisation, why a cache), not what the next line does. Match that.
- **Small commits**, with an imperative subject line —
  `Fluid: shade the pool as a reflected light ring`.
- **Tests live next to the code**, in a `#[cfg(test)] mod tests`, and their
  names read as sentences. Add one when you fix a subtle behaviour.

## Architecture decision records

Decisions that are expensive to reverse are recorded as one file per decision
in [`docs/decisions/`](./docs/decisions/), always with the same shape:
**Context, Decision, Consequences**. Write one when the decision is made, and
link it from the architecture page it affects.

> Some code cites `(ADR-0009)` for the crate's granular mode selection. That
> number refers to a decision recorded **upstream**, in the `cymatrox` project;
> the viewer's own records are numbered from `0001`.

## Submitting a pull request

A good pull request explains three things:

1. **What** changed, in one or two sentences.
2. **Why** — the bug or the need behind it.
3. **How you checked it** — the commands you ran, and what you saw on screen if
   the change is visual.

Screenshots are very welcome for anything that touches the interface. If your
change is not finished yet, open it as a draft pull request: that is the
easiest way to get early feedback.

## Debugging and logs

The app logs through `tracing`; the filter is read from `RUST_LOG`:

```bash
RUST_LOG=warn,cymatrox_viewer=info cargo run
RUST_LOG=warn,cymatrox_viewer=debug cargo run
```

## Getting help

Stuck on something? Open an issue and ask — there is no such thing as a silly
question. Please be kind and patient with the other contributors; everyone here
is doing this on their own time.

By contributing, you agree that your work is released under the
[MIT license](./LICENSE), like the rest of the project.
