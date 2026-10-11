# Change Log

All notable changes to this project will be documented in this file.

## [Unreleased]

### Physically-lit realism pass (all three modules)

Every module is now lit with real PBR materials behind one shared cinematic post-stack, instead of flat baked colour:

- **Shared cinematic stack** (`render::cinematic()`): HDR rendering, filmic Tony McMapface tonemapping and a restrained, energy-conserving bloom on the granular, fluid and acoustic cameras
- **Fluid**: the pool is now a dark, low-roughness dielectric lit by a dedicated low-angle raking key light (on the liquid layer) and a generated studio environment (`EnvironmentMapLight::hemispherical_gradient` — no HDRI asset required). The surface still carries a physical albedo (deep water, slope-driven froth, meniscus) but its brightness now comes from real diffuse/specular and the reflected environment. The displayed relief is normalised per frame, because the physical height (tens of micrometres) is far below the dish radius and would otherwise stay optically flat
- **Granular**: reworked from a flat, unlit 2D view (`granular2d`) into a lit 3D scene (`granular3d`) — a dark metal Chladni plate with the sand rendered as shaded 3D grains. A top-down orthographic camera keeps the exact pixel mapping (`scale = 1/SCALE`), so the on-screen plate size and the UI caption are unchanged, and the bright grains still read against the dark plate
- **Acoustic**: already PBR; it now shares the cinematic stack

### Analytic CymaScope figure (single stable mode)

The coarse, above-threshold fluid PDE cannot reliably settle on one Faraday figure: near-degenerate dish modes (e.g. `(9,1)` and `(6,2)` at ~30 Hz) grow together and **beat**, so the picture kept morphing instead of locking. The displayed figure is now the mode the dish selects in the weakly-driven limit — the one whose natural frequency is closest to the subharmonic `f/2` — evaluated analytically:

- **The viewer consumes the published `cymatrox = "0.2"` crate unmodified** — the local `[patch.crates-io]` fork was removed. The analytic figure lives in the viewer (`src/render/glyph.rs`: `dominant_mode`, `rasterise_glyph`, `FaradayMode`, `GLYPH_AMPLITUDE_M`)
- **The crate's fluid GPU simulation was removed from the viewer**: with the published crate there is no batched `step_n`, so feeding the display would block on a GPU readback every step (multi-second frames), and the linear solver has no above-threshold saturation. The CymaScope figure is now computed viewer-side and synchronously; the crate remains used normally for the granular and acoustic modules
- The viewer's `resolved_auto_mode` is back (the crate's `ModeSelection::Auto` scaling is not part of its public API); it only schedules the granular rescatter
- Mode shape `η(r,θ) = Jₘ(k·r)·cos(mθ)`, `k = jₘₙ/R`, with the capillary–gravity dispersion `ω² = (g·k + (σ/ρ)·k³)·tanh(k·depth)`; Bessel functions and their zeros evaluated in `f64`
- Figure is now **stable and instantaneous**, and sharpens with the tone (60 Hz → 10 petals + 2 rings; 250 Hz → 13 rings; 400 Hz → 19 rings); a tone change cross-fades to the neighbouring mode
- Render samples the glyph on a 256² field (up from the 64² solver readback), so the fine nodal network stays crisp

### UI redesign (unreleased)

- New `src/ui/` module implementing the HTML mock-up: floating rounded control card with segmented module selector, big frequency readout, section labels, and a bottom foot bar (FPS + status pill)
- Log-scale frequency ruler across the bottom of the viewport (20 Hz – 20 kHz) with accent marker at the current frequency
- Embedded DM Sans / DM Mono fonts; dark theme matching the design palette
- Renderers restyled to share the panel's dark stage and accent (`#00D2BE`), matching the design palette
- Granular: mode-aware rescatter of the grain bed when a retune lands on a different Chladni mode (debounced while the frequency is moving)
- Voice control now drives the frequency only; amplitude is left to the slider
- Default granular drive raised so patterns form quickly (`3e-4` m)

### Fluid cymascope view

- Fluid module re-presented as a top-down cymascope: orthographic camera looking straight down at the circular pool, refit to the viewport each frame so the figure never clips
- Graduated instrument bezel around the pool (dark annulus, tick marks, cardinal graduations, accent hairline) and a `CYMASCOPE` stage caption
- Adaptive polar water mesh (concentric rings) replaces the square grid, so the pool has a clean circular edge
- Mouse-wheel zoom on the scope (locked, non-orbiting view)
- Pool enlarged to fill more of the frame (`SCOPE_FRACTION` 0.48 → 0.60)

## [0.1.0] - 2026-10-07

- Initial scaffold with Bevy + egui + cymatrox integration
- Three modules: Granular (Chladni), Fluid (Faraday waves), Acoustic (pressure field)
- Real-time frame-rate-independent pacing with step accumulator
- Voice control via CPAL + realfft
- Dual-GPU-context safety (cymatrox GpuContext separate from Bevy)
- Latest-wins parameter coalescing from UI to sim thread
- Debounced simulation rebuilds to avoid churn while dragging sliders
- Tracing-based logging with configurable RUST_LOG
- CI workflow (Linux/Windows/macOS) with fmt, clippy, tests
- Packaging via cargo-packager (deb/AppImage/nsis/dmg) with auto-update support
- GPU dual-context smoke test binary (step 4b)
