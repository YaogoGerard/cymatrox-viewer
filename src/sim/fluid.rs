//! Default liquid properties for the Fluid module.
//!
//! The viewer no longer runs the crate's fluid solver: the CymaScope figure is
//! the analytic glyph in [`crate::render::glyph`], evaluated in this medium.
//! These values are therefore the physical description of the dish's liquid
//! (also used as the UI's safe fallbacks and initial values).

use cymatrox::fluid::LiquidSpec;

/// Shallow water layer on a circular laboratory dish (SI units).
pub fn default_liquid() -> LiquidSpec {
    LiquidSpec {
        density: 1000.0,
        surface_tension: 0.072,
        depth: 0.004,
        damping: 0.2,
        gravity: 9.81,
    }
}
