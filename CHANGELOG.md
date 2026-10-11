# Change Log

All notable changes to this project will be documented in this file.

## [Unreleased]

### Acoustic levitation: a legible, frequency-dependent figure

The acoustic module looked *broken*: the shared 20 Hz–20 kHz slider spent most of its travel below the 4 cm cavity's `c/2L ≈ 4.3 kHz` fundamental, the figure took ~30 s to appear, and once it did it rendered as a uniform glow that barely changed with frequency. The causes and fixes:

- **Ultrasonic band**: the levitation cell now sweeps **5–40 kHz** (its own `frequency_range`), so the whole slider travel is physically meaningful; the parameter sanitizer and the voice-control clamp follow. The ruler, scale labels and readout are band-aware (`5k / 10k / 20k / 40k`)
- **~3.75× faster settle**: `dt` was raised from the crate's conservative `4e-7 s` to `1.5e-6 s` (the CFL bound for the 32³ / 4 cm grid is ~2.1 µs), and `averaging_periods` from 8 to 3. GPU work per step is `dt`-independent, so the same readback rate advances far more simulated time — the standing wave settles in ~8 s instead of ~30 s. The step *rate* stays at the proven 60/s: higher rates starve the iGPU, so the blocking `send` back-pressure (rather than a non-blocking `try_send`) is kept
- **Single stable figure**: the solver reports the *instantaneous* pressure, which flips sign every half period. The viewer now draws a running **energy field** (p² EMA, α = 0.15) instead of the signed pressure, so the figure no longer strobes
- **Per-frame normalisation**: the undamped cavity keeps building pressure, so the old fixed colour reference saturated the whole chamber to full brightness. The volume is now normalised by its own per-frame RMS peak (floor 0.05 Pa), keeping the lobe/node ratio readable at any field level
- **The camera was staring down the band axis**: the default 3/4 orbit looked straight along the transducer axis, so the axial standing-wave bands projected onto each other and read as a solid mass. The default camera now looks **perpendicular** to that axis (yaw 0 with a slight top-down pitch), which reveals the nodal planes — an axial mode such as `(8,0,0)` at 34.3 kHz shows clean vertical bands, while a detuned drive reads as a filled diffuse field
- Voxel samples now land **on** the grid nodes (they previously sampled the half-steps *between* nodes, exactly where a standing wave's contrast is washed out), and the colour ramp drops cells below half the per-frame peak so the lobes read as sheets rather than one glow

### Fluid: a legible CymaScope figure

The pool rendered as a smooth bright dome rather than a CymaScope. At the default 60 Hz the dish selects a 10-petal, ~6-ring Faraday mode, but the display could not show it:

- **Resolution**: the polar water mesh was 36 rings × 120 segments, which aliased the radial nodal circles away — and the top of the audio range carries ~40 rings. Now 220 × 256, sampling the glyph on a 512² field (up from 256²)
- **Slope normalisation**: the displayed relief is scaled per frame so its *steepest slope* — not its peak height — is constant. A fixed height turned the high-order modes into a spiky fence; a bounded aspect keeps every tone reading as water (a few big waves at low pitch, a fine shimmer at the top)
- **Light-ring reflection**: the water is now drawn *unlit* from a baked per-vertex colour, shaded the way the instrument actually captures a cymaglyph — a coaxial **ring light** seen from straight overhead. A patch of surface glints where its tilt bends the view ray out to the ring (a contour of the wave *slope*), so the bright nodal lines and petals stand on dark water; a faint height term keeps the whole figure readable, plus slope-driven froth and a lit meniscus. This replaces the earlier crest-height + single directional caustic, which read as a painted height map rather than lit water
- The mesh is rebuilt only while the figure cross-fades and then frozen, so the finer tessellation costs nothing once a tone has settled

### Physically-lit realism pass (all three modules)

Every module is now lit with real PBR materials behind one shared cinematic post-stack, instead of flat baked colour:

- **Shared cinematic stack** (`render::cinematic()`): HDR rendering, filmic Tony McMapface tonemapping and a restrained, energy-conserving bloom on the granular, fluid and acoustic cameras
- **Fluid**: the pool is now a dark, low-roughness dielectric lit by a dedicated low-angle raking key light (on the liquid layer) and a generated studio environment (`EnvironmentMapLight::hemispherical_gradient` — no HDRI asset required). The surface still carries a physical albedo (deep water, slope-driven froth, meniscus) but its brightness now comes from real diffuse/specular and the reflected environment. The displayed relief is normalised per frame, because the physical height (tens of micrometres) is far below the dish radius and would otherwise stay optically flat
- **Granular**: reworked from a flat, unlit 2D view (`granular2d`) into a lit 3D scene (`granular3d`) — a dark metal Chladni plate with the sand rendered as shaded 3D grains. A top-down orthographic camera keeps the exact pixel mapping (`scale = 1/SCALE`), so the on-screen plate size and the UI caption are unchanged, and the bright grains still read against the dark plate
- **Acoustic**: already PBR; it now shares the cinematic stack

### Granular fluidity & sand realism

The Chladni sand read as a stutter of scattered dots rather than flowing powder:

- **Temporally filtered motion**: the renderer now chases the solver snapshots with a framerate-independent low-pass (`1 - exp(-rate·dt)`, the same trick the fluid module uses on its height field) instead of copying the newest frame verbatim, so the bed glides and settles. A solver rebuild restarts the filter cleanly rather than sliding the old bed across the plate
- **Faster physics**: the granular solver integrates one `dt = 1/480 s` per `step()`, so the old 60 steps/s ran the motion at 0.125× speed. Raised to 240 steps/s (half-speed); the existing backlog cap keeps a slow device from spiralling, so it self-limits to whatever rate the GPU sustains
- **Reconstructed pile**: a coarse density field built from the displayed grain positions raises grains where they stack on a nodal line and leans their shading normals along the pile's slope, so a ridge catches the key light and falls into shade on its flanks; crevices darken like contact shadowing
- **Real grains**: 16 384 grains (up from 4 096), each an irregular, individually sized and tinted sand chip (jittered octahedron with a graded 0.72–1.30 radius) instead of 4 096 identical pointy octahedra. Sample count is shared with the solver (`sim::granular::GRAIN_COUNT`)

### Analytic CymaScope figure (single stable mode)

The coarse, above-threshold fluid PDE cannot reliably settle on one Faraday figure: near-degenerate dish modes (e.g. `(9,1)` and `(6,2)` at ~30 Hz) grow together and **beat**, so the picture kept morphing instead of locking. The displayed figure is now the mode the dish selects in the weakly-driven limit — the one whose natural frequency is closest to the subharmonic `f/2` — evaluated analytically:

- **The viewer consumes the published `cymatrox = "0.2"` crate unmodified** — the local `[patch.crates-io]` fork was removed. The analytic figure lives in the viewer (`src/render/glyph.rs`: `dominant_mode`, `rasterise_glyph`, `FaradayMode`, `GLYPH_AMPLITUDE_M`)
- **The crate's fluid GPU simulation was removed from the viewer**: with the published crate there is no batched `step_n`, so feeding the display would block on a GPU readback every step (multi-second frames), and the linear solver has no above-threshold saturation. The CymaScope figure is now computed viewer-side and synchronously; the crate remains used normally for the granular and acoustic modules
- The viewer's `resolved_auto_mode` is back (the crate's `ModeSelection::Auto` scaling is not part of its public API); it only schedules the granular rescatter
- Mode shape `η(r,θ) = Jₘ(k·r)·cos(mθ)`, `k = jₘₙ/R`, with the capillary–gravity dispersion `ω² = (g·k + (σ/ρ)·k³)·tanh(k·depth)`; Bessel functions and their zeros evaluated in `f64`
- Figure is now **stable and instantaneous**, and sharpens with the tone (60 Hz → 10 petals + 2 rings; 250 Hz → 13 rings; 400 Hz → 19 rings); a tone change cross-fades to the neighbouring mode
- Render samples the glyph on a 256² field (up from the 64² solver readback), so the fine nodal network stays crisp

### UI redesign (unreleased)

- Interface strings are English throughout (the control panel was partly French: module names, section titles and the liquid/granular parameter labels)
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
