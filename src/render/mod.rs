pub mod acoustic3d;
pub mod camera;
pub mod fluid3d;
pub mod glyph;
pub mod granular3d;

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::Color;

/// Cinematic post-stack shared by every module camera: HDR rendering, filmic
/// (Tony McMapface) tonemapping and a restrained, energy-conserving bloom so
/// the physically-lit specular highlights bleed the way a real lens does.
///
/// Returned as one bundle so a module camera opts in with a single element:
/// `commands.spawn((Camera3d::default(), render::cinematic(), ..))`.
pub fn cinematic() -> (Hdr, Tonemapping, Bloom) {
    (
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.18,
            ..Bloom::default()
        },
    )
}

// Bevy-side mirror of `ui::theme`, lifted straight from the HTML mock-up
// (`Cymatrox — nouvelle interface.html`) so the viewports share the panel's
// palette instead of drifting into a separate "realistic" colour scheme.
//
//   stage  : radial-gradient(ellipse at 62% 45%, #13132a 0%, #0B0B14 100%)
//   accent : #00D2BE (the teal used for every figure in the design)

/// Outer stage behind the plate: the gradient's dark rim, `#0B0B14`.
pub const STAGE_BG: Color = Color::srgb(0.043, 0.043, 0.078);
/// Design accent, `#00D2BE`.
pub const ACCENT: Color = Color::srgb(0.0, 0.824, 0.745);
