# Simulation domain

The viewer drives two phenomena. Only one of them runs the `cymatrox` solver.

## Granular (Chladni)

Runs on the sim thread through `cymatrox::granular::GranularSimulation`:

- **Grains** — `GRAIN_COUNT = 16_384` per plate, a single source of truth shared
  by the solver config and the renderer so the mesh never disagrees with its
  snapshot.
- **Plate** — `PlateSpec::Idealized { side: 0.5 }`; mode selection is
  `ModeSelection::Auto`.
- **Integration** — `dt = 1/480 s`, advanced at 240 steps/s by the pacing loop.
- **Default drive** — 440 Hz, amplitude `3e-4` on the UI scale (× 1 mm).

The viewer mirrors the crate's documented mode selection
(`ω_mn = ω_base · (m² + n²) / 2`, lower-order tie-break) in `resolved_auto_mode`.
This is the one piece of crate behaviour the viewer reproduces, and only to
notice when a retune lands on a different mode so it can rescatter the bed.

## Fluid (Faraday / CymaScope)

Does **not** run a crate solver. The figure is the analytic glyph in
`render::glyph`, evaluated in the dish's liquid:

```rust
LiquidSpec {
    density: 1000.0,          // kg/m³
    surface_tension: 0.072,   // N/m
    depth: 0.004,             // m
    damping: 0.2,
    gravity: 9.81,            // m/s²
}
```

`dominant_mode` selects the standing mode nearest the Faraday subharmonic `f/2`;
`rasterise_glyph` renders it, peak-normalised to `GLYPH_AMPLITUDE_M`. See
`render/glyph.rs` and [decisions/0002](decisions/0002-analytic-fluid-figure.md).

## Units and the amplitude sliders

Two constants convert the normalised UI `amplitude` (0–1) to physical
quantities:

| Constant | Meaning |
| --- | --- |
| `GRANULAR_AMPLITUDE_M = 0.001` | Plate displacement per unit of UI amplitude (metres). |
| `FLUID_AMPLITUDE_MPS2 = 40.0` | Acceleration the Fluid slider represents (m/s²). The figure is normalised, so this is a unit label. |

`SimParams::sanitized()` clamps and finite-checks every field before it reaches
the solver, so a bad UI value can never propagate:

| Field | Range |
| --- | --- |
| `frequency_hz` | 20 – 20 000 |
| `amplitude` | 0 – 1 |
| `drag` | 0 – 20 |
| `restitution` | 0 – 1 |
| `coupling_k` | 1 – 1e7 |
| `fluid_density` | 800 – 1500 |
| `fluid_surface_tension` | 0.01 – 0.5 |
| `fluid_depth` | 0.001 – 0.02 |
| `fluid_damping` | 0 – 2 |

---

[← Previous: Data flow](data-flow.md) · [Next: Rendering →](rendering.md)
