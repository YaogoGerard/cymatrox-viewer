# Change Log

All notable changes to this project will be documented in this file.

## [Unreleased]

### UI redesign

- New `src/ui/` module implementing the HTML mock-up: floating rounded control card with segmented module selector, big frequency readout, section labels, and a bottom foot bar (FPS + status pill)
- Log-scale frequency ruler across the bottom of the viewport (20 Hz – 20 kHz) with accent marker at the current frequency
- Embedded DM Sans / DM Mono fonts; dark theme matching the design palette
- Renderers restyled to share the panel palette: stage/plate colours and teal accent (`#00D2BE`) applied to granular, fluid and acoustic viewports
- Granular: mode-aware rescatter of the grain bed when a retune lands on a different Chladni mode (debounced while the frequency is moving)
- Voice control now drives the frequency only; amplitude is left to the slider
- Default granular drive raised so patterns form quickly (`3e-4` m)

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
