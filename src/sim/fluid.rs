use cymatrox::fluid::{
    DomainMask, DomainShape, Driving, FluidConfig, LiquidSpec, SolverParams, SurfaceGrid,
};

pub fn default_config() -> FluidConfig {
    // These values describe a shallow water layer on a circular laboratory
    // dish; they are deliberately kept in SI units for reproducibility.
    FluidConfig {
        driving: Driving {
            frequency_hz: 60.0,
            amplitude: 2.0,
        },
        liquid: LiquidSpec {
            density: 1000.0,
            surface_tension: 0.072,
            depth: 0.004,
            damping: 0.8,
            gravity: 9.81,
        },
        surface: SurfaceGrid {
            width: 128,
            height: 128,
            extent: [0.06, 0.06],
            readback_stride: 2,
            noise_amplitude: 1e-3,
            seed: 42,
        },
        domain: DomainMask {
            shape: DomainShape::Circular { radius: 0.025 },
        },
        solver: SolverParams { dt: 4e-5 },
    }
}
