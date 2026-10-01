use bevy::prelude::*;

pub mod acoustic;
pub mod fluid;
pub mod granular;

/// Number of fluid sub-steps run per master-loop iteration so that
/// sim-time moves fast enough for Faraday instability to build visible
/// ripples (each blocking GPU step is expensive on iGPUs).
const FLUID_STEPS_PER_FRAME: usize = 80;

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

/// Data sent from the sim thread back to Bevy.
pub enum SimData {
    Granular(Vec<cymatrox::granular::GranularData>),
    Fluid(Vec<cymatrox::fluid::FluidSurfaceNode>),
    Acoustic(Vec<cymatrox::acoustic::AcousticPressureNode>),
    Status(String),
}

#[derive(Resource)]
pub struct SimDataReceiver {
    pub rx: crossbeam_channel::Receiver<SimData>,
}

#[derive(Resource)]
pub struct SimParamsSender {
    pub tx: crossbeam_channel::Sender<SimParams>,
}

#[derive(Resource)]
pub struct SimControl {
    pub switch_tx: crossbeam_channel::Sender<crate::ActiveModule>,
}

pub fn start_sim_thread() -> (SimDataReceiver, SimParamsSender, SimControl) {
    let (data_tx, data_rx) = crossbeam_channel::bounded::<SimData>(4);
    let (params_tx, params_rx) = crossbeam_channel::bounded::<SimParams>(2);
    let (switch_tx, switch_rx) = crossbeam_channel::bounded::<crate::ActiveModule>(1);

    let _handle = spawn_master(params_rx, data_tx, switch_rx);

    (
        SimDataReceiver { rx: data_rx },
        SimParamsSender { tx: params_tx },
        SimControl { switch_tx },
    )
}

fn spawn_master(
    params_rx: crossbeam_channel::Receiver<SimParams>,
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
                    eprintln!("[sim] {message}");
                    let _ = data_tx.send(SimData::Status(message));
                    return;
                }
            };
            let ctx = match rt.block_on(cymatrox::GpuContext::new()) {
                Ok(ctx) => ctx,
                Err(e) => {
                    let message = format!("GPU initialization failed: {e}");
                    eprintln!("[sim] {message}");
                    let _ = data_tx.send(SimData::Status(message));
                    return;
                }
            };

            use crate::ActiveModule::*;
            let mut active = Granular;
            let mut granular: Option<cymatrox::granular::GranularSimulation> =
                match cymatrox::granular::GranularSimulation::new(&ctx, granular::default_config()) {
                    Ok(simulation) => Some(simulation),
                    Err(e) => {
                        let message = format!("Granular initialization failed: {e}");
                        eprintln!("[sim] {message}");
                        let _ = data_tx.send(SimData::Status(message));
                        return;
                    }
                };
            let mut fluid: Option<cymatrox::fluid::FluidSimulation> = None;
            let mut acoustic: Option<cymatrox::acoustic::AcousticSimulation> = None;
            let mut last_freq: f32 = 440.0;
            let mut _last_amp: f32 = 0.0001;
            let mut last_drag: f32 = 4.0;
            let mut last_restitution: f32 = 0.6;
            let mut last_coupling_k: f32 = 5.0e5;
            let mut last_fluid_density: f32 = 1000.0;
            let mut last_fluid_st: f32 = 0.072;
            let mut last_fluid_depth: f32 = 0.004;
            let mut last_fluid_damping: f32 = 0.8;
            let mut last_step_log: Option<std::time::Instant> = None;
            let s_step_dt: f32 = fluid::default_config().solver.dt;
            eprintln!("[sim] granular ready");

            loop {
                while let Ok(new_active) = switch_rx.try_recv() {
                    if new_active != active {
                        match new_active {
                            Granular if granular.is_none() => {
                                match cymatrox::granular::GranularSimulation::new(
                                    &ctx,
                                    granular::default_config(),
                                ) {
                                    Ok(s) => {
                                        granular = Some(s);
                                        eprintln!("[sim] granular ready");
                                    }
                                    Err(e) => {
                                        let message = format!("Granular initialization failed: {e}");
                                        eprintln!("[sim] {message}");
                                        let _ = data_tx.send(SimData::Status(message));
                                    }
                                }
                            }
                            Fluid => {
                                match cymatrox::fluid::FluidSimulation::new(&ctx, fluid::default_config()) {
                                    Ok(s) => { fluid = Some(s); eprintln!("[sim] fluid ready"); }
                                    Err(e) => { eprintln!("[sim] fluid init failed: {e}"); }
                                }
                            }
                            Acoustic if acoustic.is_none() => {
                                match cymatrox::acoustic::AcousticSimulation::new(&ctx, acoustic::default_config()) {
                                    Ok(s) => { acoustic = Some(s); eprintln!("[sim] acoustic ready"); }
                                    Err(e) => { eprintln!("[sim] acoustic init failed: {e}"); }
                                }
                            }
                            _ => {}
                        }
                        active = new_active;
                    }
                }

                while let Ok(p) = params_rx.try_recv() {
                    // --- Granular ---
                    let frequency_hz = if p.frequency_hz.is_finite() {
                        p.frequency_hz.clamp(20.0, 20_000.0)
                    } else {
                        last_freq
                    };
                    let amplitude = if p.amplitude.is_finite() {
                        p.amplitude.clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let density = if p.fluid_density.is_finite() {
                        p.fluid_density.clamp(800.0, 1500.0)
                    } else {
                        last_fluid_density
                    };
                    let surface_tension = if p.fluid_surface_tension.is_finite() {
                        p.fluid_surface_tension.clamp(0.01, 0.5)
                    } else {
                        last_fluid_st
                    };
                    let depth = if p.fluid_depth.is_finite() {
                        p.fluid_depth.clamp(0.001, 0.02)
                    } else {
                        last_fluid_depth
                    };
                    let damping = if p.fluid_damping.is_finite() {
                        p.fluid_damping.clamp(0.0, 2.0)
                    } else {
                        last_fluid_damping
                    };
                    let rel_freq_change = if last_freq > 0.0 {
                        (frequency_hz - last_freq).abs() / last_freq
                    } else {
                        f32::INFINITY
                    };
                    let freq_changed = rel_freq_change > 0.1;
                    let solver_changed = (p.drag - last_drag).abs() > f32::EPSILON
                        || (p.restitution - last_restitution).abs() > f32::EPSILON
                        || (p.coupling_k - last_coupling_k).abs() > f32::EPSILON;
                    let needs_rebuild = freq_changed || solver_changed;

                    if needs_rebuild && active == Granular {
                        let mut cfg = granular::default_config();
                        cfg.experiment.frequency_hz = frequency_hz;
                        cfg.experiment.amplitude = amplitude * crate::GRANULAR_AMPLITUDE_M;
                        cfg.solver.drag = p.drag;
                        cfg.solver.restitution = p.restitution;
                        cfg.solver.coupling_k = p.coupling_k;
                        match cymatrox::granular::GranularSimulation::new(&ctx, cfg) {
                            Ok(s) => { granular = Some(s); eprintln!("[sim] granular reset freq={:.1} drag={:.1} rest={:.2} ck={:.0}", frequency_hz, p.drag, p.restitution, p.coupling_k); }
                            Err(e) => { eprintln!("[sim] granular reset failed: {e}"); }
                        }
                    } else {
                        if let Some(ref mut s) = granular {
                            s.set_frequency(frequency_hz);
                            s.set_amplitude(amplitude * crate::GRANULAR_AMPLITUDE_M);
                        }
                    }

                    // --- Fluid: amplitude in m/s² (UI sends 0..1).
                    // The Mathieu `g_eff = g·h + a·h·cos(ωt)` coefficient goes
                    // negative once peak accel `a` exceeds g≈9.81, which makes
                    // the linear discritization blow up unphysically (observed:
                    // |h| → 1e23). Cap the slider range so a ∈ [0, ~9] m/s²,
                    // keeping g_eff positive while still above the Faraday
                    // threshold (~0.5–1 g).
                    let fluid_amp = amplitude * crate::FLUID_AMPLITUDE_MPS2;
                    let fluid_freq_changed = (frequency_hz - last_freq).abs() / last_freq.max(0.1) > 0.2;
                    let liquid_changed = (density - last_fluid_density).abs() > f32::EPSILON
                        || (surface_tension - last_fluid_st).abs() > f32::EPSILON
                        || (depth - last_fluid_depth).abs() > f32::EPSILON
                        || (damping - last_fluid_damping).abs() > f32::EPSILON;
                    let fluid_needs_rebuild = fluid_freq_changed || liquid_changed;

                    if fluid_needs_rebuild && active == Fluid {
                        let cfg = cymatrox::fluid::FluidConfig {
                            driving: cymatrox::fluid::Driving {
                                frequency_hz,
                                amplitude: fluid_amp,
                            },
                            liquid: cymatrox::fluid::LiquidSpec {
                                density,
                                surface_tension,
                                depth,
                                damping,
                                gravity: 9.81,
                            },
                            ..fluid::default_config()
                        };
                        match cymatrox::fluid::FluidSimulation::new(&ctx, cfg) {
                            Ok(s) => { fluid = Some(s); eprintln!("[sim] fluid reset freq={:.1} amp={:.1}", frequency_hz, fluid_amp); }
                            Err(e) => {
                                let message = format!("Fluid reset failed: {e}");
                                eprintln!("[sim] {message}");
                                let _ = data_tx.send(SimData::Status(message));
                            }
                        }
                    } else {
                        if let Some(ref mut s) = fluid {
                            s.set_frequency(frequency_hz);
                            s.set_amplitude(fluid_amp);
                        }
                    }

                    if let Some(ref mut s) = acoustic {
                        s.set_frequency(frequency_hz);
                        s.set_amplitude(amplitude * crate::ACOUSTIC_AMPLITUDE_MPS);
                    }

                    last_freq = frequency_hz;
                    _last_amp = p.amplitude;
                    last_drag = p.drag;
                    last_restitution = p.restitution;
                    last_coupling_k = p.coupling_k;
                    last_fluid_density = density;
                    last_fluid_st = surface_tension;
                    last_fluid_depth = depth;
                    last_fluid_damping = damping;
                }

                // Each module owns its physical time step. The UI thread only
                // receives snapshots, so rendering cannot alter the solver.
                let data = match active {
                    Granular => granular.as_mut().and_then(|s| match s.step() {
                        Ok(frame) => Some(SimData::Granular(frame)),
                        Err(e) => {
                            eprintln!("[sim] granular step failed: {e}");
                            None
                        }
                    }),
                    Fluid => {
                        // Bulk-step so sim-time accumulates enough drive
                        // periods for Faraday waves to grow (blocking GPU
                        // readback is slow on iGPU hardware).
                        let mut frame = None;
                        if let Some(ref mut s) = fluid {
                            for _ in 0..FLUID_STEPS_PER_FRAME {
                                match s.step() {
                                    Ok(f) => frame = Some(f),
                                    Err(e) => {
                                        eprintln!("[sim] fluid step failed: {e}");
                                        break;
                                    }
                                }
                            }
                            let now = std::time::Instant::now();
                            if last_step_log
                                .as_ref()
                                .map(|t| now.duration_since(*t) >= std::time::Duration::from_secs(1))
                                .unwrap_or(true)
                            {
                                eprintln!(
                                    "[sim] fluid: {} steps/frame, dt={:.0e}s, sim-time/frame={:.3}s",
                                    FLUID_STEPS_PER_FRAME,
                                    s_step_dt,
                                    FLUID_STEPS_PER_FRAME as f32 * s_step_dt
                                );
                                last_step_log = Some(now);                            }
                            if let Some(f) = frame.take() {
                                Some(SimData::Fluid(f))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    Acoustic => acoustic.as_mut().and_then(|s| match s.step() {
                        Ok(frame) => Some(SimData::Acoustic(frame)),
                        Err(e) => {
                            eprintln!("[sim] acoustic step failed: {e}");
                            None
                        }
                    }),
                };

                if let Some(d) = data {
                    if data_tx.send(d).is_err() {
                        break;
                    }
                }
            }
        })
        .expect("failed to spawn master sim thread")
}
