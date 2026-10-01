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
            dt: 4e-7,
            averaging_periods: 8.0,
        },
    }
}
