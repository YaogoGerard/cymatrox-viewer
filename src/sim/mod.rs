use bevy::log::{debug, error, info};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub mod acoustic;
pub mod fluid;
pub mod granular;

/// Target simulation steps per wall-clock second, per module.
///
/// These match the previous fixed per-frame step counts at ~60 FPS, but are
/// now decoupled from the render frame rate: the physics advances at the same
/// rate whether the app renders at 30, 60 or 144 FPS.
///
/// The granular solver integrates one `dt = 1/480 s` per `step()`, so the step
/// rate sets how fast the sand actually flows against the wall clock. The old
/// 60 steps/s ran the physics at 0.125× speed — a slow creep that reads as
/// stutter, not motion. 240 steps/s pulls it to a natural half-speed while the
/// backlog cap below keeps a slow GPU from spiralling (it simply self-limits to
/// whatever rate the device sustains).
const GRANULAR_STEPS_PER_SEC: f64 = 240.0;
/// The acoustic solver integrates one `dt = 1.5e-6 s` per `step()` (see
/// `acoustic::default_config`), so 60 steps/s advances the levitation cell
/// ~3.75x faster than the crate's reference pacing while keeping the readback
/// rate low enough for the renderer to keep up on the UHD 630 iGPU (higher
/// rates starve rendering — verified experimentally).
const ACOUSTIC_STEPS_PER_SEC: f64 = 60.0;

/// Largest backlog of simulation steps the accumulator may hold. Without this
/// cap, a single GPU step slower than its budget would grow the backlog without
/// bound and trigger an ever-worsening catch-up spiral.
const MAX_BACKLOG_SECONDS: f64 = 0.1;

/// How long solver parameters must remain stable before the affected
/// simulation is rebuilt. Prevents a rebuild on every frame while a slider is
/// being dragged.
const REBUILD_DEBOUNCE: Duration = Duration::from_millis(300);

/// Relative/absolute tolerance under which two solver values are considered
/// unchanged, so identical parameter messages never trigger a rebuild.
const PARAM_EPSILON: f32 = 1e-6;

/// Normalized UI amplitude converted to plate displacement in metres.
pub const GRANULAR_AMPLITUDE_M: f32 = 0.001;
/// Acceleration amplitude (m/s²) the Fluid module's amplitude slider represents.
/// The figure itself is the normalised analytic glyph, so this is a unit label.
pub const FLUID_AMPLITUDE_MPS2: f32 = 40.0;
/// Normalized UI amplitude converted to transducer normal velocity (m/s).
pub const ACOUSTIC_AMPLITUDE_MPS: f32 = 0.002;

/// Parameters sent from the UI thread to the sim thread.
#[derive(Clone, Debug)]
pub struct SimParams {
    pub frequency_hz: f32,
    pub amplitude: f32,
    pub drag: f32,
    pub restitution: f32,
    pub coupling_k: f32,
    pub fluid_density: f32,
    pub fluid_surface_tension: f32,
    pub fluid_depth: f32,
    pub fluid_damping: f32,
}

impl SimParams {
    /// Clamps and finite-checks every field so a bad UI value can never reach
    /// the solver. Values that fail the finite check fall back to the safe
    /// physical defaults of the corresponding module config.
    pub fn sanitized(self) -> Self {
        let g = granular::default_config();
        let f = fluid::default_liquid();
        let sanitize = |value: f32, fallback: f32, min: f32, max: f32| {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                fallback
            }
        };
        Self {
            // Upper bound is the acoustic module's ultrasonic band (see
            // `crate::frequency_range`); the audible modules clamp lower in the
            // UI, so this only has to be a safety net against bad input.
            frequency_hz: sanitize(self.frequency_hz, g.experiment.frequency_hz, 20.0, 40_000.0),
            amplitude: sanitize(self.amplitude, 0.0, 0.0, 1.0),
            drag: sanitize(self.drag, g.solver.drag, 0.0, 20.0),
            restitution: sanitize(self.restitution, g.solver.restitution, 0.0, 1.0),
            coupling_k: sanitize(self.coupling_k, g.solver.coupling_k, 1.0, 1.0e7),
            fluid_density: sanitize(self.fluid_density, f.density, 800.0, 1500.0),
            fluid_surface_tension: sanitize(
                self.fluid_surface_tension,
                f.surface_tension,
                0.01,
                0.5,
            ),
            fluid_depth: sanitize(self.fluid_depth, f.depth, 0.001, 0.02),
            fluid_damping: sanitize(self.fluid_damping, 0.2, 0.0, 2.0),
        }
    }
}

impl Default for SimParams {
    fn default() -> Self {
        let g = granular::default_config();
        let f = fluid::default_liquid();
        Self {
            frequency_hz: g.experiment.frequency_hz,
            amplitude: g.experiment.amplitude / GRANULAR_AMPLITUDE_M,
            drag: g.solver.drag,
            restitution: g.solver.restitution,
            coupling_k: g.solver.coupling_k,
            fluid_density: f.density,
            fluid_surface_tension: f.surface_tension,
            fluid_depth: f.depth,
            fluid_damping: 0.2,
        }
    }
}

/// Data sent from the sim thread back to Bevy.
pub enum SimData {
    Granular(Vec<cymatrox::granular::GranularData>),
    Acoustic(Vec<cymatrox::acoustic::AcousticPressureNode>),
    Status(String),
}

#[derive(Resource)]
pub struct SimDataReceiver {
    pub rx: crossbeam_channel::Receiver<SimData>,
}

/// Latest-wins parameter slot shared between the UI and the sim thread.
///
/// A single slot cannot fill up like a bounded channel: the UI never blocks and
/// a burst of changes collapses to the most recent value instead of being
/// silently dropped.
#[derive(Resource, Clone)]
pub struct SimParamsSlot(pub Arc<Mutex<Option<SimParams>>>);

impl SimParamsSlot {
    pub fn set(&self, params: SimParams) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(params);
        }
    }
}

#[derive(Resource)]
pub struct SimControl {
    pub switch_tx: crossbeam_channel::Sender<crate::ActiveModule>,
}

pub fn start_sim_thread() -> (SimDataReceiver, SimParamsSlot, SimControl) {
    let (data_tx, data_rx) = crossbeam_channel::bounded::<SimData>(4);
    let (switch_tx, switch_rx) = crossbeam_channel::bounded::<crate::ActiveModule>(1);
    let slot = Arc::new(Mutex::new(None));
    let _handle = spawn_master(slot.clone(), data_tx, switch_rx);

    (
        SimDataReceiver { rx: data_rx },
        SimParamsSlot(slot),
        SimControl { switch_tx },
    )
}

/// The Chladni mode the crate's `ModeSelection::Auto` picks for a frequency,
/// under its documented ideal scaling `ω_mn = ω_base · (m² + n²) / 2` with a
/// lower-order tie-break (ADR-0009).
///
/// The viewer needs to know when a frequency retune lands on a different mode
/// (see the reseed note in `spawn_master`). The crate computes this internally
/// but does not expose it, so we mirror the documented scaling here; it is the
/// only piece of crate behaviour we reproduce, and only to schedule a rescatter.
fn resolved_auto_mode(frequency_hz: f32, base_frequency_hz: f32) -> (u32, u32) {
    let mut best = (1u32, 1u32);
    let mut best_err = f32::INFINITY;
    for m in 1..=24u32 {
        for n in 1..=24u32 {
            let err = (base_frequency_hz * (m * m + n * n) as f32 * 0.5 - frequency_hz).abs();
            if err < best_err - f32::EPSILON
                || ((err - best_err).abs() <= f32::EPSILON && m + n < best.0 + best.1)
            {
                best_err = err;
                best = (m, n);
            }
        }
    }
    best
}

/// Builds a granular config from the current parameters.
fn granular_config_from(params: &SimParams) -> cymatrox::granular::GranularConfig {
    let mut cfg = granular::default_config();
    cfg.experiment.frequency_hz = params.frequency_hz;
    cfg.experiment.amplitude = params.amplitude * GRANULAR_AMPLITUDE_M;
    cfg.solver.drag = params.drag;
    cfg.solver.restitution = params.restitution;
    cfg.solver.coupling_k = params.coupling_k;
    cfg
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= PARAM_EPSILON * a.abs().max(b.abs()).max(1.0)
}

fn take_params(slot: &Arc<Mutex<Option<SimParams>>>) -> Option<SimParams> {
    slot.lock().ok().and_then(|mut guard| guard.take())
}

fn spawn_master(
    params_slot: Arc<Mutex<Option<SimParams>>>,
    data_tx: crossbeam_channel::Sender<SimData>,
    switch_rx: crossbeam_channel::Receiver<crate::ActiveModule>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("cymatrox-master".into())
        .spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let message = format!("Tokio runtime unavailable: {e}");
                    error!("{message}");
                    let _ = data_tx.send(SimData::Status(message));
                    return;
                }
            };
            let ctx = match rt.block_on(cymatrox::GpuContext::new()) {
                Ok(ctx) => ctx,
                Err(e) => {
                    let message = format!("GPU initialization failed: {e}");
                    error!("{message}");
                    let _ = data_tx.send(SimData::Status(message));
                    return;
                }
            };

            let report = |message: String| {
                error!("{message}");
                let _ = data_tx.send(SimData::Status(message));
            };

            use crate::ActiveModule::*;

            let mut params = SimParams::default();
            let mut active = Granular;

            let default_granular = granular::default_config();
            let mut built_solver = (
                default_granular.solver.drag,
                default_granular.solver.restitution,
                default_granular.solver.coupling_k,
            );

            // The plate rim is nodal for *every* mode, so grains that drift onto
            // it never come back. Left alone, a few frequency sweeps pile the
            // whole bed on the rim and no figure forms again. So whenever a
            // retune lands on a different mode we rescatter the bed (rebuild the
            // solver) and let the new figure settle. `built_mode` mirrors the
            // crate's Auto selection; `reseed_since` debounces the rebuild while
            // the frequency is still moving (slider drag, voice control).
            let base_frequency_hz = default_granular.solver.base_frequency_hz;
            let mut built_mode = resolved_auto_mode(params.frequency_hz, base_frequency_hz);
            let mut reseed_armed = false;
            let mut reseed_since = Instant::now();

            let mut granular: Option<cymatrox::granular::GranularSimulation> =
                match cymatrox::granular::GranularSimulation::new(&ctx, granular_config_from(&params))
                {
                    Ok(simulation) => {
                        info!("[sim] granular ready");
                        Some(simulation)
                    }
                    Err(e) => {
                        report(format!("Granular initialization failed: {e}"));
                        None
                    }
                };
            let mut acoustic: Option<cymatrox::acoustic::AcousticSimulation> = None;

            let mut last_tick = Instant::now();
            let mut accumulator: f64 = 0.0;
            let mut last_param_change = Instant::now();

            loop {
                // --- Module switches -------------------------------------------------
                while let Ok(new_active) = switch_rx.try_recv() {
                    if new_active == active {
                        continue;
                    }
                    match new_active {
                        Granular if granular.is_none() => {
                            match cymatrox::granular::GranularSimulation::new(
                                &ctx,
                                granular_config_from(&params),
                            ) {
                                Ok(s) => {
                                    granular = Some(s);
                                    built_solver = (
                                        params.drag,
                                        params.restitution,
                                        params.coupling_k,
                                    );
                                    built_mode =
                                        resolved_auto_mode(params.frequency_hz, base_frequency_hz);
                                    reseed_armed = false;
                                    info!("[sim] granular ready");
                                }
                                Err(e) => report(format!("Granular initialization failed: {e}")),
                            }
                        }
                        Acoustic if acoustic.is_none() => {
                            match cymatrox::acoustic::AcousticSimulation::new(
                                &ctx,
                                acoustic::default_config(),
                            ) {
                                Ok(s) => {
                                    acoustic = Some(s);
                                    info!("[sim] acoustic ready");
                                }
                                Err(e) => report(format!("Acoustic initialization failed: {e}")),
                            }
                        }
                        _ => {}
                    }
                    active = new_active;
                    last_tick = Instant::now();
                    accumulator = 0.0;
                }

                // --- Latest parameters ----------------------------------------------
                if let Some(received) = take_params(&params_slot) {
                    params = received.sanitized();
                    last_param_change = Instant::now();

                    let granular_amp = params.amplitude * GRANULAR_AMPLITUDE_M;
                    let acoustic_amp = params.amplitude * ACOUSTIC_AMPLITUDE_MPS;
                    if let Some(s) = granular.as_mut() {
                        s.set_frequency(params.frequency_hz);
                        s.set_amplitude(granular_amp);
                    }
                    if let Some(s) = acoustic.as_mut() {
                        s.set_frequency(params.frequency_hz);
                        s.set_amplitude(acoustic_amp);
                    }
                }

                // --- Debounced rebuild ----------------------------------------------
                let now = Instant::now();
                let solver_differs = !approx_eq(params.drag, built_solver.0)
                    || !approx_eq(params.restitution, built_solver.1)
                    || !approx_eq(params.coupling_k, built_solver.2);

                // A mode change asks for a rescatter, but only once the
                // frequency has settled, so dragging the slider (or sweeping
                // with the voice) doesn't rebuild the solver on every frame.
                let mode_changed = active == Granular
                    && resolved_auto_mode(params.frequency_hz, base_frequency_hz) != built_mode;
                if mode_changed && !reseed_armed {
                    reseed_armed = true;
                    reseed_since = now;
                } else if !mode_changed {
                    reseed_armed = false;
                }
                let debounce_elapsed =
                    now.duration_since(last_param_change) >= REBUILD_DEBOUNCE;
                let mode_reseed_due =
                    reseed_armed && now.duration_since(reseed_since) >= REBUILD_DEBOUNCE;

                let rebuild_due = match active {
                    Granular => (solver_differs && debounce_elapsed) || mode_reseed_due,
                    Fluid | Acoustic => false,
                };

                if rebuild_due {
                    match active {
                        Granular => {
                            match cymatrox::granular::GranularSimulation::new(
                                &ctx,
                                granular_config_from(&params),
                            ) {
                                Ok(s) => {
                                    granular = Some(s);
                                    built_solver = (
                                        params.drag,
                                        params.restitution,
                                        params.coupling_k,
                                    );
                                    built_mode =
                                        resolved_auto_mode(params.frequency_hz, base_frequency_hz);
                                    reseed_armed = false;
                                    debug!(
                                        "[sim] granular rebuilt drag={:.1} rest={:.2} ck={:.0} mode={:?}",
                                        params.drag,
                                        params.restitution,
                                        params.coupling_k,
                                        built_mode
                                    );
                                }
                                Err(e) => {
                                    report(format!("Granular rebuild failed: {e}"));
                                }
                            }
                        }
                        Fluid | Acoustic => {}
                    }
                    // Avoid re-triggering until parameters change again.
                    last_param_change = now;
                }

                // --- Frame-rate-independent pacing ----------------------------------
                let steps_per_sec = match active {
                    Granular => GRANULAR_STEPS_PER_SEC,
                    Acoustic => ACOUSTIC_STEPS_PER_SEC,
                    // No solver: the CymaScope figure is computed synchronously
                    // viewer-side, so there is nothing to pace.
                    Fluid => 0.0,
                };
                let elapsed = now.duration_since(last_tick).as_secs_f64();
                last_tick = now;
                accumulator += elapsed * steps_per_sec;
                let max_backlog = steps_per_sec * MAX_BACKLOG_SECONDS;
                if accumulator > max_backlog {
                    accumulator = max_backlog;
                }
                let has_sim = match active {
                    Granular => granular.is_some(),
                    Acoustic => acoustic.is_some(),
                    Fluid => false,
                };
                let mut steps = accumulator.floor() as usize;
                if has_sim {
                    accumulator -= steps as f64;
                } else {
                    // No solver yet (still initializing or failed): drop the
                    // backlog so it cannot burst once the solver appears.
                    accumulator = 0.0;
                    steps = 0;
                }

                // Each module owns its physical time step. The UI thread only
                // receives snapshots, so rendering cannot alter the solver.
                let mut latest: Option<SimData> = None;
                match active {
                    Granular => {
                        if let Some(sim) = granular.as_mut() {
                            for _ in 0..steps {
                                match sim.step() {
                                    Ok(frame) => latest = Some(SimData::Granular(frame)),
                                    Err(e) => {
                                        report(format!("Granular step failed: {e}"));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // No solver: the CymaScope figure is viewer-side and
                    // synchronous, so there is nothing to step.
                    Fluid => {}
                    Acoustic => {
                        if let Some(sim) = acoustic.as_mut() {
                            for _ in 0..steps {
                                match sim.step() {
                                    Ok(frame) => latest = Some(SimData::Acoustic(frame)),
                                    Err(e) => {
                                        report(format!("Acoustic step failed: {e}"));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(data) = latest {
                    // Blocking send doubles as the render's back-pressure: when
                    // the channel is full the sim thread waits, which hands the
                    // GPU queue back to the renderer. Without it the solver
                    // would read back continuously and starve rendering.
                    if data_tx.send(data).is_err() {
                        debug!("[sim] UI disconnected, stopping sim thread");
                        break;
                    }
                }

                // Nothing to do this iteration: yield instead of busy-spinning.
                if steps == 0 {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        })
        .expect("failed to spawn master sim thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_track_module_configs() {
        let params = SimParams::default();
        let g = granular::default_config();
        let f = fluid::default_liquid();
        assert!((params.frequency_hz - g.experiment.frequency_hz).abs() < f32::EPSILON);
        assert!((params.drag - g.solver.drag).abs() < f32::EPSILON);
        assert!((params.restitution - g.solver.restitution).abs() < f32::EPSILON);
        assert!((params.coupling_k - g.solver.coupling_k).abs() < f32::EPSILON);
        assert!((params.fluid_density - f.density).abs() < f32::EPSILON);
        assert!((params.fluid_depth - f.depth).abs() < f32::EPSILON);
        // Normalized amplitude round-trips back to the physical driving value.
        assert!((params.amplitude * GRANULAR_AMPLITUDE_M - g.experiment.amplitude).abs() < 1e-9);
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let raw = SimParams {
            frequency_hz: 1.0e9,
            amplitude: 5.0,
            drag: -3.0,
            restitution: 4.0,
            coupling_k: 0.0,
            fluid_density: 10.0,
            fluid_surface_tension: 9.0,
            fluid_depth: 0.0,
            fluid_damping: 100.0,
        };
        let s = raw.sanitized();
        assert_eq!(s.frequency_hz, 40_000.0);
        assert_eq!(s.amplitude, 1.0);
        assert_eq!(s.drag, 0.0);
        assert_eq!(s.restitution, 1.0);
        assert_eq!(s.coupling_k, 1.0);
        assert_eq!(s.fluid_density, 800.0);
        assert_eq!(s.fluid_surface_tension, 0.5);
        assert_eq!(s.fluid_depth, 0.001);
        assert_eq!(s.fluid_damping, 2.0);
    }

    #[test]
    fn sanitize_replaces_non_finite_with_defaults() {
        let raw = SimParams {
            frequency_hz: f32::NAN,
            amplitude: f32::INFINITY,
            drag: f32::NAN,
            restitution: f32::NAN,
            coupling_k: f32::NAN,
            fluid_density: f32::NAN,
            fluid_surface_tension: f32::NAN,
            fluid_depth: f32::NAN,
            fluid_damping: f32::NAN,
        };
        let s = raw.sanitized();
        let d = SimParams::default();
        assert!(s.frequency_hz.is_finite() && s.frequency_hz == d.frequency_hz);
        assert!(s.drag.is_finite() && s.drag == d.drag);
        assert!(s.fluid_density.is_finite() && s.fluid_density == d.fluid_density);
    }

    #[test]
    fn approx_eq_treats_identical_and_distinct_values_correctly() {
        assert!(approx_eq(4.0, 4.0));
        assert!(approx_eq(0.0, 0.0));
        assert!(!approx_eq(0.0, 1.0));
        assert!(!approx_eq(1000.0, 1000.5));
    }
}
