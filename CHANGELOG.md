# Change Log

All notable changes to this project will be documented in this file.

## [Unreleased]

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
