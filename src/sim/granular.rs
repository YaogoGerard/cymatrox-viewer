use cymatrox::granular::{
    Driving, GrainBed, GranularConfig, InitialDistribution, ModeSelection, PlateSpec, SolverParams,
};

pub fn default_config() -> GranularConfig {
    // The idealized plate keeps the solver deterministic; measured plate
    // eigenmodes can be introduced later through ModeSelection::Measured.
    GranularConfig {
        experiment: Driving {
            frequency_hz: 440.0,
            amplitude: 1e-4,
            modes: ModeSelection::Auto,
        },
        medium: PlateSpec::Idealized { side: 0.5 },
        grains: GrainBed {
            count: 4096,
            distribution: InitialDistribution::Uniform,
            seed: 123,
        },
        solver: SolverParams {
            dt: 1.0 / 480.0,
            drag: 4.0,
            restitution: 0.6,
            coupling_k: 5.0e5,
            base_frequency_hz: 120.0,
        },
    }
}
