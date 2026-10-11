use cymatrox::acoustic::{
    AcousticConfig, Axis, Driving, MediumSpec, ParticleSpec, Side, SolverParams, TransducerSpec,
    VolumeGrid,
};

pub fn default_config() -> AcousticConfig {
    // Acoustic driving is a transducer surface velocity (m/s), not pressure.
    // The viewer converts its normalized slider before reaching this config.
    AcousticConfig {
        driving: Driving {
            frequency_hz: 24_000.0,
            amplitude: 5.0,
        },
        medium: MediumSpec {
            density: 1.2041,
            sound_speed: 343.0,
        },
        volume: VolumeGrid {
            width: 32,
            height: 32,
            depth: 32,
            extent: [0.04; 3],
            readback_stride: 2,
            noise_amplitude: 1e-9,
            seed: 123,
        },
        transducer: TransducerSpec {
            axis: Axis::X,
            side: Side::Low,
        },
        particle: ParticleSpec {
            radius: 1e-3,
            density: 1000.0,
            sound_speed: 1480.0,
        },
        solver: SolverParams {
            // The CFL bound for the 4 cm / 32³ grid is ~2.1e-6 s (ADR-0012).
            // 1.5e-6 keeps a 1.4x margin while advancing physical time 3.75x
            // faster per step than the crate's conservative reference, so the
            // standing wave settles in ~8 s instead of ~30 s. The step *rate*
            // stays at the proven 60/s — higher rates starve the renderer.
            dt: 1.5e-6,
            // Eight periods of cycle averaging made the Gor'kov force take ~20 s
            // to converge; three still smooths the acoustic cycle but settles
            // far sooner.
            averaging_periods: 3.0,
        },
    }
}
