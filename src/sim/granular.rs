use cymatrox::granular::{
    Driving, GrainBed, GranularConfig, InitialDistribution, ModeSelection, PlateSpec, SolverParams,
};

/// Sand grains simulated (and drawn) per plate.
///
/// Single source of truth shared by the solver config and the renderer, so the
/// mesh never disagrees with the snapshot it is fed. Dense enough that the
/// figure reads as a continuous powder ridge rather than scattered dots.
pub const GRAIN_COUNT: u32 = 16_384;

pub fn default_config() -> GranularConfig {
    // The idealized plate keeps the solver deterministic; measured plate
    // eigenmodes can be introduced later through ModeSelection::Measured.
    GranularConfig {
        experiment: Driving {
            frequency_hz: 440.0,
            // 0.30 on the UI scale (×1 mm). A softer starting drive than the
            // mock-up's 0.60: the grains creep onto the nodal lines instead of
            // slamming into them, so the figure sharpens more gradually and
            // reads less "painted". Still 3× the original 0.10, which was so
            // weak the pattern took minutes to appear at all.
            amplitude: 3e-4,
            modes: ModeSelection::Auto,
        },
        medium: PlateSpec::Idealized { side: 0.5 },
        grains: GrainBed {
            count: GRAIN_COUNT,
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
