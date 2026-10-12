# Rendering

Everything visual is Bevy. Each module owns a camera and a renderer; both share
one cinematic post-stack and one palette.

## Cameras and layers

- A **module viewport camera** per 3D module renders that module into its
  viewport. Orbit input is handled manually (`render::camera`) so egui's
  full-screen context cannot swallow input meant for the 3D view.
- **Render layers** keep the modules from leaking into each other: the granular
  plate and stage sit on layers `0, 1, 3` (lit by the shared key light); the
  fluid dish uses its own layer `2` with a dedicated low raking light.
- The **egui camera** is a second camera (`order 1`, `RenderLayers::none()`) so
  the panel composites over the 3D views.

## Post-stack

`render::cinematic()` returns one bundle every module camera opts into:

- `Hdr`
- `Tonemapping::TonyMcMapface` (filmic)
- `Bloom { intensity: 0.18 }` (restrained, energy-conserving)

## Renderers

| Renderer | Module | Notes |
| --- | --- | --- |
| `granular3d.rs` | Granular | A physically-lit aluminium plate under sand, kept as a **top-down orthographic** view so the Chladni figure stays as legible as the old flat 2D version. `SCALE` maps one world unit to a fixed number of pixels; `PLATE_HALF_PX` feeds the UI caption. |
| `fluid3d.rs` | Fluid | A top-down orthographic *CymaScope*: a graduated instrument bezel around a circular dish whose surface is the analytic glyph, shaded as the reflection of a coaxial light ring. While a tone is held the surface keeps moving — travelling capillary ripples and a slow slosh — and a tone change grows the new figure in over about a second. |
| `glyph.rs` | Fluid | The analytic Faraday rasteriser: Bessel-based mode selection and a peak-normalised glyph. Pure and unit-tested in place. |

The palette (`STAGE_BG`, `ACCENT`) mirrors `ui::theme`, so the viewports and the
panel share one colour scheme instead of drifting apart.

---

[← Previous: Simulation domain](simulation.md) · [Next: Control panel →](ui.md)
