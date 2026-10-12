# 0002 — Compute the Fluid figure analytically, viewer-side

- Status: accepted

## Context

A CymaScope imprints sound on a shallow water layer: the surface is driven
vertically and, above a threshold, a stationary Faraday pattern locks in.

A thin-film PDE on a coarse grid cannot *reliably* pick that single figure: for
this dish two nearly degenerate modes (e.g. `(9, 1)` and `(6, 2)` at ~30 Hz)
grow together and beat, so the picture keeps morphing instead of settling.
Driving the crate's fluid solver in the viewer would also require a per-step GPU
readback, which would stall the display every frame.

## Decision

Do not run a fluid solver in the viewer. Compute the CymaScope figure
**analytically and synchronously** on the main thread, in `render::glyph`:

- `dominant_mode` selects the mode whose natural frequency is closest to the
  Faraday subharmonic `f / 2`. In the weakly-driven limit that is the most
  resonant mode — the highest growth rate, and the only one to survive.
- `rasterise_glyph` evaluates that Bessel mode (`h(r,θ) = J_m(j_{m,n} r / R) ·
  cos(mθ)`), peak-normalised to `GLYPH_AMPLITUDE_M`.

## Consequences

- No per-frame readback; the figure is essentially free at render time.
- The figure is deterministic and unit-testable (Bessel zeros, monotonic
  fineness, amplitude bounds) without a GPU.
- The viewer no longer exercises the crate's fluid solver: the liquid properties
  remain only as the medium the glyph is evaluated in, and the crate's fluid
  solver is treated as a separate, general-purpose simulation.

---

[← Previous: ADR 0001](0001-two-gpu-contexts.md) · [Next: ADR 0003 →](0003-keep-cymatrox-unmodified.md)
