# COMPREHENSIVE PROMPT: Modern egui UI Redesign for cymatrox-viewer

## OBJECTIVE
Generate a complete, production-ready redesign of the cymatrox-viewer control panel UI (egui, Bevy) that is modern, functional, coherent with the existing application. The redesign must be 100% implementable in egui, preserve every behavioral detail, and be refactored cleanly into src/ui/ without breaking the simulation architecture.

## 1. MUST READ THESE FILES FIRST
Before proposing changes, extract exact semantics from:
- src/main.rs — UiState struct (all fields), SimState (granular_data/fluid_data/acoustic_data/status/error), ActiveModule enum+label, PrevModule, ModuleDefaultsApplied, constants (ACCENT, BG_*, TEXT_*, PANEL_*, etc.), main() systems (order: EguiPrimaryContextPass for ui_panel, Update chain with poll/sync/render/diagnose), setup_scene (egui camera + lighting + layers), start_sim/start_audio, poll_voice (mapping + scaling), poll_sim_data (match arms, sets error on Status), diagnose (120-frame cadence), sync_params/sync_module_switch, ui_panel (365–724), styled_card/slider_row
- src/sim/mod.rs — SimParams (clone/debug + sanitized() + Default impl), SimData enum, SimDataReceiver, SimParamsSlot (Arc<Mutex<Option<SimParams>>> with set()), SimControl, GRANULAR_STEPS_PER_SEC/ACOUSTIC_STEPS_PER_SEC/FLUID_STEPS_PER_SEC, MAX_BACKLOG_SECONDS, REBUILD_DEBOUNCE (Duration 300ms), PARAM_EPSILON, physical constants (GRANULAR_AMPLITUDE_M 1e-3, FLUID_AMPLITUDE_MPS2 10.0, ACOUSTIC_AMPLITUDE_MPS 2e-3), approx_eq, take_params, spawn_master (threading, tokio runtime, ctx creation, init order, switch handling, param hot-updates, debounced rebuilds, accumulator pacing, backlog cap, yield)
- src/sim/{granular,fluid,acoustic}.rs — default_config() values, SI units, solver.dt
- src/audio/mod.rs — VoiceData, sample buffer (1024), window Hann, dominant bin, RMS, 20ms sleep
- src/render/{camera.rs,granular2d.rs,fluid3d.rs,acoustic3d.rs,mod.rs} — viewport offset, orbit controls, visibility layers
- Cargo.toml, README.md, design_spec.md, ui_system_brief.txt

## 2. APPLICATION MODEL (DEEP DIVE)

### Modules
- ActiveModule: Granular(default), Fluid, Acoustic. label() returns strings.

### UI state (src/main.rs)
UiState fields: active_module, frequency_hz, amplitude (normalized 0–1), drag, restitution, coupling_k, fluid_density, fluid_surface_tension, fluid_depth, fluid_damping, voice_control(bool), voice_freq, voice_rms, fps. Default derived from sim::SimParams::default().

### Sim state
SimState: granular_data: Option<Vec<GranularData>>, fluid_data: Option<Vec<FluidSurfaceNode>>, acoustic_data: Option<Vec<AcousticPressureNode>>, status: String, error: bool. Status messages come from sim thread on init/rebuild/step failures.

### Physical mappings
- Granular amplitude UI 0–1 * GRANULAR_AMPLITUDE_M=0.001 -> plate displacement meters. UI displays "×1 mm".
- Fluid amplitude UI 0–1 * FLUID_AMPLITUDE_MPS2=10.0 -> m/s² vertical acceleration. UI "×10 m/s²".
- Acoustic amplitude UI 0–1 * ACOUSTIC_AMPLITUDE_MPS=0.002 -> transducer normal velocity m/s. UI "×0.002 m/s".

### Pacing (critical)
Accumulator-based, frame-rate independent:
- steps/sec: GRANULAR 60, ACOUSTIC 60, FLUID 4800
- elapsed = now-last_tick; accumulator += elapsed*steps_per_sec; cap = steps_per_sec*MAX_BACKLOG_SECONDS(0.1); steps = floor(accum); if no sim ready: accumulator=0, steps=0; after stepping subtract steps as f64; if steps==0 sleep 1ms

### Parameter flow
UI change -> sync_params detects ui_state.is_changed() -> params_slot.set(SimParams::from(&ui_state)) (latest-wins). Sim thread take_params() once/iteration, calls .sanitized(), sets hot-updates (set_frequency/amplitude on all active sims immediately), records last_param_change=now. Debounced rebuild: if active module’s solver/liquid differs (approx_eq with PARAM_EPSILON) AND now - last_param_change >= REBUILD_DEBOUNCE (300ms) -> rebuild that simulation with config_from, update built_* caches, debug log, reset last_param_change=now.

### Module defaults
ModuleDefaultsApplied{fluid,acoustic}. First time switching to Fluid (and not applied): set freq 60.0, amp 0.20, defaults.fluid=true. First time to Acoustic: freq 24000.0, amp 0.20, defaults.acoustic=true. Granular uses defaults from SimParams.

### Voice
poll_voice reads VoiceReceiver.rx.try_recv() while available. If voice_control true and values finite: freq = clamp(20,20000), amp = (rms*0.01).clamp(0.00001,1.0). Voice thread captures mono, 1024 buffer, Hann window, realfft, peak bin -> freq_hz, RMS.

## 3. CURRENT UI ANALYSIS
Left egui Panel::left, responsive_panel_width. Sections: Title "CYMATROX" + subtitle, Error banner (if sim_state.error), MODULE selector (3 buttons), DRIVING (Frequency log 20–20000 + Amplitude with unit suffix per module), SOLVER (collapsible, Granular only), LIQUID (collapsible, Fluid only), VOICE (checkbox + readouts), STATUS (FPS, streaming/starting dot+pill, module, frame counts). Dark theme, accent cyan #00D2BE.

## 4. REDESIGN PRINCIPLES
- Preserve 100% behavior (timings, mappings, semantics, ranges, order of systems).
- Modern instrument panel: clarity, hierarchy, reduced noise, coherence.
- Card-based, segmented controls, inline value+unit, clear states.
- Implementable in pure egui, no new dependencies.
- Refactor to src/ui/{theme.rs,widgets.rs,panel.rs,mod.rs} cleanly.

## 5. VISUAL LANGUAGE (DETAILED)
Palette (dark-first):
- Background/base #0B0B14, panel #12121E (keep compatibility), surface/card #151522, surface-hover #1B1B2B, border #242438 (40–60% alpha), accent #00D2BE, accent-dim #008C82, text-primary #F4F4F8, text-secondary #B9B9C9, text-tertiary #8C8CA0, success #00D2BE, warning #F2A65A, error #D65454, error-bg #461818

Spacing 4pt: xs 4, sm 8, md 12, lg 16, xl 20, 2xl 24. Radius 10 (cards), 6–8 (buttons). Typography: H1 22–24 bold, H2 13–14 bold, Body 11–12, Value 12 medium, Caption 10–11.

## 6. COMPONENT SPECS (PRECISE)
- ModuleTabs: horizontal segmented, equal widths, active filled accent (text #0B0B14), inactive card bg, text-secondary. Updates ui_state.active_module immediately.
- GroupCard: Frame fill surface/card, radius 10, inner_margin md, stroke border alpha 0.4, header (H2) + vertical body with item_spacing sm.
- SliderRow: top row label (secondary) + value+unit (right, Value). bottom slider full width. frequency logarithmic true, others false. units exactly as current per module.
- StatusPill: if error -> red ERROR. else if frame present for active module (granular_data/fluid_data/acoustic_data Some) -> cyan STREAMING. else amber STARTING. dot + text.
- ErrorBanner: only if error, red bg/border, title + status text (wrap).
- VoiceReadout: caption style when enabled.

## 7. LAYOUT HIERARCHY
1. Header (title+subtitle)
2. ModuleTabs
3. ErrorBanner (conditional)
4. DRIVING card (Frequency + Amplitude)
5. PHYSICS card (contextual: Granular -> Solver 3 sliders; Fluid -> Liquid 4 sliders; Acoustic -> empty/hidden)
6. AUDIO card (voice checkbox + readouts)
7. STATUS card (FPS left, pill right, module + frame stats below)

## 8. IMPLEMENTATION REQUIREMENTS
- Create src/ui/theme.rs (palette + apply_dark_theme(ctx)), widgets.rs (helpers returning Responses as appropriate), panel.rs (ui_panel), mod.rs (pub mod exports). 
- Move ui_panel/styled_card/slider_row/responsive_panel_width? responsive_panel_width pub in main (camera uses). Either keep in main or re-export. Prefer minimal disruption.
- Keep system order unchanged: EguiPrimaryContextPass calls ui_panel; Update chain identical.
- No allocations in UI hot path. Use format! minimally; prefer inline RichText.
- Apply theme once per frame via style_mut_of(Theme::Dark,...)
- Preserve debounce 300ms, latest-wins, pacing, all mappings.

## 9. OUTPUT DELIVERABLE
Return full, final implementation: (a) all 4 ui/*.rs files with complete, final code; (b) main.rs diffs (minimal edits: add mod ui, replace ui_panel call/use, remove moved helpers if any); (c) ASCII wireframes per 3 states; (d) color tokens as code constants; (e) acceptance checklist with verification notes.

Be exhaustive, precise, production-ready. No placeholder stubs. Output in English.
