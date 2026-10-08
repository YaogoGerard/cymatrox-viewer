# PROMPT: Redesign Cymatrox Viewer UI (egui)

Paste this entire file as the system/user prompt to an AI tasked with redesigning the UI.

## CONTEXT
You are redesigning the control panel UI for cymatrox-viewer, a Rust/Bevy+egui real-time cymatics lab. The app renders GPU simulations from cymatrox (separate crate) across three domains. The UI must be modern, functional, coherent, and 100% implementable in egui without changing the GUI stack.

## PROJECT FILES TO READ
Read these files to understand exact implementation:
- src/main.rs (UiState, SimState, ActiveModule, panel, systems: sync_params/sync_module_switch/poll_sim_data/poll_voice, constants, responsive_panel_width)
- src/sim/mod.rs (SimParams, SimData, SimParamsSlot, start_sim_thread, pacing with accumulator, debounce REBUILD_DEBOUNCE=300ms, MAX_BACKLOG_SECONDS, steps/sec per module)
- src/sim/granular.rs / fluid.rs / acoustic.rs (defaults, scales)
- src/audio/mod.rs (CPAL+realfft, VoiceData)
- src/render/*.rs (cameras, viewports, granular2d/fluid3d/acoustic3d)
- Cargo.toml
- README.md, design_spec.md, ui_system_brief.txt

## EXACT DOMAIN BEHAVIOR

### Granular (Chladni)
- 4096 grains, idealized plate. Renders 2D instanced. Color = velocity magnitude.
- Amplitude mapping: UI normalized (0–1) -> physical plate displacement GRANULAR_AMPLITUDE_M = 1e-3 m. UI shows "×1 mm" in current code (meaning 1.0 = 1mm). 
- Params: frequency 20–20000 Hz (log slider), amplitude 0–1, drag 0–20, restitution 0–1, coupling_k 1–1e7 N/m.
- hot-updates: freq/amplitude applied immediately via sim.set_frequency/set_amplitude. solver changes debounced 300ms -> full GranularSimulation::new rebuild.

### Fluid (Faraday)
- Circular dish, heightmap 3D. Smoothed over time/spatially.
- Amplitude mapping: UI -> FLUID_AMPLITUDE_MPS2 = 10.0 m/s². UI shows "×10 m/s²".
- Params: frequency 20–20000, amplitude 0–1, density 800–1500 kg/m³, surface_tension 0.01–0.5 N/m, depth 0.001–0.02 m, damping 0–2.
- Defaults applied once on first switch to Fluid (60 Hz, 0.20). 
- Fluid steps/sec high (4800) to grow instability; pacing is fixed-step accumulator (independent of FPS). 
- Liquid changes debounced 300ms -> rebuild.

### Acoustic
- Sparse voxel volume, chamber, transducers, particle. Volume rebuilt throttled ~20Hz in render.
- Amplitude mapping: UI -> ACOUSTIC_AMPLITUDE_MPS = 2e-3 m/s. Shows "×0.002 m/s".
- Defaults applied once on first switch to Acoustic (24000 Hz, 0.20). 
- freq/amplitude hot-updated; acoustic has no solver/liquid group.

## THREADING & STATE (CRITICAL)
- sim thread owns cymatrox::GpuContext (separate from Bevy wgpu). 
- SimParamsSlot is Arc<Mutex<Option<SimParams>>>: latest-wins. UI writes on change; sim takes() once per iteration. Never block UI.
- SimControl.switch_tx sends ActiveModule on change (bounded 1). 
- SimDataReceiver gets SimData::{Granular,Fluid,Acoustic,Status}. 
- SimState: granular_data/fluid_data/acoustic_data Option<Vec<...>>, status String, error bool.
- VoiceReceiver gets VoiceData{freq_hz, amplitude} at ~50Hz; poll_voice maps to UI when enabled (clamp 20–20000, scale rms to 0.00001–1.0).
- Pacing: accumulator += elapsed*steps_per_sec; cap by steps_per_sec*MAX_BACKLOG_SECONDS (0.1s); floor steps; if no sim, zero backlog. Yield when steps==0.

## UI CONSTRAINTS TO PRESERVE
- Left egui Panel::left, responsive width via responsive_panel_width(window_logical_width)= (w*0.30).clamp(220,300)
- Viewport offset for 3D cameras uses panel width (must remain correct)
- RenderLayers + camera visibility logic unchanged
- All ranges, units strings, logarithmic flags, clamping semantics
- Debounce 300ms timing
- Latest-wins semantics
- Module switch order/behavior
- Voice mapping
- Error banner visibility when error=true; status shows FPS + streaming/starting + frame counts
- fps pulled from FrameTimeDiagnosticsPlugin

## DESIGN GOALS
Modern, minimal, coherent scientific instrument. Prioritize scannability and reducing visual noise. Use dark theme with subtle depth. Card-based groups. Segmented module tabs. Inline value+unit on sliders. Clear status pill (cyan streaming / amber starting / red error). Prominent error banner. Typography hierarchy.

## TARGET STRUCTURE (egui)
Refactor ui_panel into src/ui/{theme.rs, widgets.rs, panel.rs, mod.rs}:
- theme.rs: color palette (evolve slightly: bg #0B0B14, card #151522, border #242438, accent #00D2BE, text #F4F4F8/#B9B9C9/#8C8CA0, error states). Apply via ctx.style_mut_of(Theme::Dark,...)
- widgets.rs: ModuleTabs, GroupCard, SliderRow(label,value,range,unit,log), StatusPill, ErrorBanner, VoiceReadout
- panel.rs: ui_panel moved from main.rs, uses helpers
- mod.rs: exports

Keep logic unchanged. Avoid allocations in UI hot path.

## OUTPUT REQUIREMENTS
Produce a complete, final answer with:
1. Detailed color palette + spacing + typography + radius
2. New left panel hierarchy
3. Per-component specs (props, layout, states)
4. Wireframe for Granular/Fluid/Acoustic states (ASCII + description)
5. State mapping rules
6. Concrete egui code for theme.rs, widgets.rs, panel.rs (minimal but complete enough to drop in) respecting exact IDs/semantics
7. Migration diff guidance for main.rs (minimal edits)
8. Acceptance checklist
9. Notes about viewport/camera, debounce, pacing, latest-wins

Be explicit and pragmatic. Do not change physics/rendering. Output in English.
