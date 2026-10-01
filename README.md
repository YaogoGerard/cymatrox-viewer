# cymatrox-viewer

Native desktop app replacing the earlier website plan. `cymatrox` stays
untouched — this depends on it (`cymatrox = "0.2"`) as an external
consumer, same as `export-frames/` did for the website.

## Architecture

- **egui (via `bevy_egui`)** for UI — runs directly inside Bevy's render
  pipeline, no separate GUI toolkit/adapter (this replaced an earlier
  Slint-hosts-Bevy design specifically to avoid a third GPU context).
- **Bevy** for 3D rendering.
- **cymatrox** on its own `GpuContext` (wgpu 30), independent of Bevy's
  render device, driven from a dedicated sim thread.
- **cpal + realfft** for optional voice control (mic → frequency/amplitude).
- **crossbeam-channel** connecting the sim thread, audio thread, and Bevy.

## Status: scaffold and initial integration

What's here: the app builds a window, Bevy cameras, an egui control panel,
three simulation/rendering paths, and optional microphone FFT input. The
simulation and rendering paths are integrated, but still need production
hardening and performance profiling.

## Implementation order

| # | Task | Status |
|---|------|--------|
| 1 | Scaffold `Cargo.toml` + `main.rs` | done |
| 2 | egui panel + placeholder controls | done |
| 3 | ~~Slint↔Bevy adapter~~ | dropped — egui runs in-pipeline |
| 4 | Bevy 3D camera setup | done |
| 4b | Dual-GPU-context smoke test (Bevy's wgpu + cymatrox's `GpuContext::new()` side by side, no rendering) | **not done — do this before step 6** |
| 5 | Audio capture + FFT | done (initial) |
| 6 | cymatrox sim thread | done (initial) |
| 7 | Granular 2D/3D rendering | done (initial) |
| 8 | UI ↔ sim callbacks | done (initial) |
| 9 | Fluid 3D rendering | done (initial) |
| 10 | Acoustic 3D rendering | done (initial) |
| 11 | Voice control | done (initial) |
| 12 | Polish | not started |

Step 4b was added on top of the original plan — it's the highest-risk
unknown (two separate wgpu devices, one process, likely an integrated
GPU) and cheap to test in isolation before building three more layers
on top of it.
