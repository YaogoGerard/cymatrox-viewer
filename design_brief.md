# Cymatrox Viewer — Design Brief for AI (Interface/UX Redesign)

## Goal
Redesign the UI of cymatrox-viewer into a more modern, functional, coherent interface while preserving all current functionality. The new design must be implementable in egui (or with minimal changes to the Bevy+egui stack). Keep it production-ready and maintainable.

## What this app is
An interactive cymatics laboratory built in Rust with Bevy + egui. It visualizes real-time GPU simulations from the `cymatrox` crate (headless physics) across three regimes:

1. **Granular (Chladni)** — 4096 grains on an idealized vibrating plate. Renders as 2D instanced hex-like grains; color encodes velocity magnitude. Physical mapping: UI amplitude (0–1) × GRANULAR_AMPLITUDE_M (1e-3 m) plate displacement. Controls: Frequency (Hz, log), Amplitude (×1 mm display), Drag, Restitution, Coupling k (N/m).
2. **Fluid (Faraday waves)** — Shallow fluid in a circular dish, driven vertically. Renders 3D circular dish + heightmap (deformed in Y). Time-smoothed + lightly spatially smoothed. Physical mapping: amplitude × FLUID_AMPLITUDE_MPS2 (10 m/s²). Controls: Frequency (Hz), Amplitude (×10 m/s²), Density (kg/m³), Surface tension (N/m), Depth (m), Damping. Simulation runs at fixed physics steps/sec (independent of FPS); defaults (60 Hz, 0.20) trigger visible Faraday growth.
3. **Acoustic (standing pressure field)** — 3D acoustic pressure in a chamber with transducers. Renders sparse voxel volume (only |p|>threshold), translucent chamber, two transducer discs with glowing rims, levitating particle. Throttled volume rebuild (~20 Hz). Physical mapping: amplitude × ACOUSTIC_AMPLITUDE_MPS (2e-3 m/s) transducer velocity. Controls: Frequency (Hz, often ultrasonic), Amplitude (×0.002 m/s).

## Architecture (critical to preserve)
- **Multi-threaded**: main Bevy thread (render/UI), dedicated "cymatrox-master" sim thread (owns `cymatrox::GpuContext` wgpu device, separate from Bevy), audio thread (CPAL + realfft → VoiceData).
- **Data flow**: UI → latest-wins `SimParams` slot (no bounded channel overflow) → sim thread sanitizes/clamps → applies hot-updates (`set_frequency`/`set_amplitude`) immediately; solver/liquid changes are **debounced** (300ms stable) before rebuilding simulation (prevents churn while dragging). Granular/Acoustic persist across switches; Fluid may be recreated on demand; module defaults applied once per first-visit.
- **Time pacing**: frame-rate independent — accumulator uses `dt_solver`-based steps/sec per module (not "1 step per frame"). Max backlog cap prevents spiral.
- **Channels/resources**: `crossbeam-channel` (bounded 1–4), `SimDataReceiver`, `SimParamsSlot`, `SimControl(switch_tx)`, `VoiceReceiver`, `SimState` (latest frames + status + error flag).
- **Viewport/camera**: left egui panel (responsive width), 3D modules use PanOrbitCamera with viewport offset (panel width), granular 2D. RenderLayers isolate per-module visibility.
- **Voice control**: mic FFT (dominant bin) maps freq (20–20k Hz) and RMS (scaled) to UI params when enabled.

## Current UI (egui)
Left sidebar panel with sections: CYMATROX title/subtitle, MODULE (3 buttons: Granular/Fluid/Acoustic), DRIVING (Frequency + Amplitude with unit label), SOLVER (collapsible, Granular only: Drag, Restitution, Coupling k), LIQUID (collapsible, Fluid only: Density, Surface tension, Depth, Damping), VOICE (checkbox + live freq/RMS), STATUS (FPS, STREAMING/STARTING, module name, frame size). Dark theme with accent cyan. Error banner appears on `sim_state.error`.

## Design goals for new UI
Create a **modern, functional, coherent** redesign that feels like a desktop scientific instrument/control panel but polished. Prioritize clarity, minimalism, and discoverability without hiding critical controls.

Requirements:
- Preserve all controls exactly (ranges/units semantics) — Frequency log slider 20–20k, amplitudes as normalized with correct unit suffixes, solver/liquid ranges as is.
- Keep module switching instant and obvious. Clearly show which module is active and which context-specific controls are visible.
- Show live status: FPS, connection/streaming state, frame stats (grains/nodes/cells), simulation errors prominently.
- Support voice control toggle + live readout.
- Maintain left sidebar layout or propose a better layout that still leaves most viewport to 3D/2D render (don’t shrink viewport unnecessarily). Consider resizable panel? Current is fixed-width responsive.
- Use egui — don’t introduce new GUI stack. Respect existing color tokens (accent cyan #00D2BE) or evolve subtly while staying readable on dark bg.
- Keep accessibility to key params without excessive nesting. Collapsibles for SOLVER/LIQUID are fine but could be tabs/groups.
- Avoid breaking the debounced rebuild semantics or time pacing.
- Show units clearly next to values/sliders.
- Error states must be visible (banner + status color).
- Don’t remove diagnostics (FPS) — useful.

Suggested modern direction (inspiration, not copy):
- **Header**: compact logo + module selector as segmented control/tabs (Granular | Fluid | Acoustic) with active state.
- **Card-based groups**: “Driving”, “Physics”, “Audio/Control”, “Status”. Soft borders, subtle hover, rounded corners.
- **Sliders**: with value display inline, unit label, logarithmic where appropriate (frequency). Consider “compact” mode but keep precision.
- **Status pill**: colored dot (green streaming, amber starting, red error) + text.
- **Error panel**: full-width card with icon/text when `error` true.
- **Subtle typography**: smaller labels, clear hierarchy.
- **Optional**: “Reset to defaults” per group/module? (fluid defaults exist on first switch; could expose reset). But not required.
- **Theme**: dark with glassy subtle cards, accent for active/interactive. Avoid pure black; current BG_DARK #12121E is good base.

## Deliverable for AI
Return a **complete, implementable design spec** including:
1. **Visual design language**: color palette (with hex), spacing scale (4pt grid), typography scale, border radius, elevation/shadow, icon ideas.
2. **Layout structure**: new left panel hierarchy, sectioning, responsive behavior.
3. **Component specs**: Module tabs, SliderRow (with inline value/unit/log), CollapsibleCard vs GroupCard, StatusPill, ErrorBanner, Checkbox, Voice readout.
4. **Wireframe description**: ASCII or detailed layout of each state (Granular/Fluid/Acoustic) showing which sections appear.
5. **State representation**: how to reflect streaming/error/starting, voice enabled, active module.
6. **egui implementation guidance**: concrete widget structure, styling via `ctx.style_mut_of(Theme::Dark, ...)` or per-widget, how to avoid allocations in hot path, preserve debounced rebuild logic.
7. **Migration plan**: step-by-step to refactor `ui_panel()` (src/main.rs ~365–724) into `src/ui/` (panel.rs, theme.rs, widgets.rs, mod.rs) without breaking behavior.
8. **Acceptance checklist**: verify ranges/units, debounce timing unchanged, latest-wins params, voice mapping unchanged, viewport offset unchanged, module switch semantics unchanged.
9. **Minimal example**: a small egui snippet showing the new module tabs + driving card to prove feasibility.

Keep answer in English (matches codebase). Focus on **pragmatic, implementable** changes — not a full Figma, but something an agent can code in egui directly.
