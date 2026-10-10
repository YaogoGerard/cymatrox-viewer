pub mod acoustic3d;
pub mod camera;
pub mod fluid3d;
pub mod granular2d;

use bevy::prelude::Color;

// Bevy-side mirror of `ui::theme`, lifted straight from the HTML mock-up
// (`Cymatrox — nouvelle interface.html`) so the viewports share the panel's
// palette instead of drifting into a separate "realistic" colour scheme.
//
//   stage  : radial-gradient(ellipse at 62% 45%, #13132a 0%, #0B0B14 100%)
//   plate  : #13132a field, #242438 hairline frame
//   accent : #00D2BE (the teal used for every figure in the design)

/// Outer stage behind the plate: the gradient's dark rim, `#0B0B14`.
pub const STAGE_BG: Color = Color::srgb(0.043, 0.043, 0.078);
/// Plate field: the gradient's bright centre, `#13132a`.
pub const PLATE_BG: Color = Color::srgb(0.075, 0.075, 0.165);
/// Hairline plate frame, `#242438`.
pub const FRAME: Color = Color::srgb(0.141, 0.141, 0.220);
/// Design accent, `#00D2BE`.
pub const ACCENT: Color = Color::srgb(0.0, 0.824, 0.745);
