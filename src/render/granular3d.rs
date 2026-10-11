//! Granular (Chladni) module, rendered as a physically-lit 3D scene.
//!
//! Earlier this module was a flat, unlit 2D view: crisp, but stylised. It is now
//! a real aluminium plate covered in sand and lit by the shared key light and a
//! generated environment. To keep the Chladni figure just as legible as before —
//! dark plate, bright grains piling on the nodal lines — the camera stays a
//! *top-down orthographic* view and is scaled so one world unit is exactly
//! `SCALE` pixels, matching the old 2D mapping (so `PLATE_HALF_PX`, used by the
//! UI caption, remains correct).
//!
//! Two things make the sand read as sand rather than as a scatter of dots:
//!
//! * **Fluidity.** The solver publishes snapshots on its own thread; the
//!   renderer chases them with a framerate-independent temporal filter (the same
//!   trick the fluid module uses on its height field), so the bed glides and
//!   settles instead of snapping from one snapshot to the next.
//! * **A reconstructed pile.** Thousands of grains that pile on a node are far
//!   denser than the rest of the plate. From the displayed positions we build a
//!   coarse density field, use it to raise the grains (they physically stack on
//!   the ridges) and to tilt their shading normals, and darken the crevices the
//!   way contact shadowing darkens a real powder. Each grain is also an
//!   irregular, individually sized and tinted speck, so the pile never reads as
//!   a stamped pattern.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::render::STAGE_BG;
use crate::sim::granular::GRAIN_COUNT;
use crate::{ActiveModule, SimState, UiState};

const LAYER: RenderLayers = RenderLayers::layer(1);

#[derive(Component)]
pub struct GranularCamera;

#[derive(Component)]
pub struct GranularParticles;

/// Plate side in world units. With [`SCALE`] this is the 600 px plate of the
/// design; the sand coordinates are expressed in these same units.
const PLATE_SIZE: f32 = 0.5;
/// Pixels per world unit. Kept identical to the old 2D mapping so the on-screen
/// geometry and the UI caption are unchanged.
const SCALE: f32 = 1200.0;
/// Half the on-screen plate side, in display pixels (`PLATE_SIZE * SCALE`),
/// central to the module viewport. Shared with the UI so the material caption
/// can be pinned just beside the plate's corner.
pub const PLATE_HALF_PX: f32 = PLATE_SIZE * SCALE * 0.5;
/// Number of grains drawn. The solver is configured with the same count (see
/// [`crate::sim::granular::GRAIN_COUNT`]).
const COUNT: usize = GRAIN_COUNT as usize;

/// Plate thickness in world units (a thin slab, not a sheet of paper).
const PLATE_THICKNESS: f32 = 0.012;
/// Grain radius in display pixels, converted to world units through [`SCALE`].
/// Smaller than before (and with more grains) so the figure reads as a fine
/// powder ridge instead of a grid of coarse beads.
const GRAIN_RADIUS_PX: f32 = 1.4;
const GRAIN_RADIUS: f32 = GRAIN_RADIUS_PX / SCALE;

/// Parking offset for grains that have no simulation data yet (kept far outside
/// the orthographic frustum instead of drawing a clump at the origin).
const OFFSCREEN: f32 = -1000.0;

/// Pile reconstruction grid (`GRID × GRID` cells over the plate). Coarse enough
/// that a node accumulates many grains per cell, which is what makes its ridge
/// stand out.
const GRID: usize = 72;
/// A cell is "a full pile" at this many times the mean density. Below it the
/// shade/tilt blends linearly; above it they saturate.
const PILE_DENSITY_REF: f32 = 2.4;
/// Vertical rise of a full pile, in world units (a small physical swell: sand
/// stacked on the nodal lines really is thicker there).
const PILE_HEIGHT: f32 = 0.006;
/// How strongly the pile's slope leans the grain normals, and the cap that keeps
/// the lean from turning into a hard edge.
const TILT_GAIN: f32 = 0.18;
const MAX_TILT: f32 = 0.35;
/// Sand brightness across a pile: crevices (bare plate) sit at `MIN` and the
/// crest of a ridge at `MIN + GAIN`.
const PILE_MIN_SHADE: f32 = 0.82;
const PILE_SHADE_GAIN: f32 = 0.36;

/// Display filter rate (1/s) for the temporal chase of the solver snapshots.
/// High enough that fast kicks are followed closely, low enough that the bed
/// visibly flows between frames. Framerate-independent (`1 - exp(-rate·dt)`).
const SMOOTH_RATE: f32 = 14.0;

/// Unit octahedron used as the sand-grain template. Six vertices, smooth
/// (radial) normals, eight faces — enough to catch the key light as a rounded
/// speck. Each grain perturbs these into an irregular shape at setup.
const OCTA_AXES: [[f32; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];
const OCTA_FACES: [[u32; 3]; 8] = [
    [0, 2, 4],
    [0, 4, 3],
    [0, 3, 5],
    [0, 5, 2],
    [1, 4, 2],
    [1, 3, 4],
    [1, 5, 3],
    [1, 2, 5],
];

/// Per-grain geometry and colour, generated once. Kept out of the mesh so the
/// per-frame rebuild is a plain translate of fixed offsets.
#[derive(Resource)]
pub struct GrainProfile {
    /// Vertex offsets from the grain centre (6 per grain, `COUNT * 6` long).
    offsets: Vec<[f32; 3]>,
    /// Shading normals for those vertices.
    normals: Vec<[f32; 3]>,
    /// Rest height of each grain centre, so an irregular shape still touches the
    /// plate instead of floating or sinking.
    rest: Vec<f32>,
    /// Per-grain albedo tint (warm, slightly varied sand).
    tints: Vec<[f32; 4]>,
}

/// Tiny deterministic RNG (SplitMix64) so the grain bed is identical every run
/// and reproducible in tests.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// Builds one irregular, individually sized, tinted grain per index.
fn grain_profile() -> GrainProfile {
    let mut rng = Rng::new(0x0C1C_A7E5_5EED_1234);
    let mut offsets = Vec::with_capacity(COUNT * 6);
    let mut normals = Vec::with_capacity(COUNT * 6);
    let mut rest = Vec::with_capacity(COUNT);
    let mut tints = Vec::with_capacity(COUNT);

    for _ in 0..COUNT {
        // Sand is graded: a spread of grain sizes, not one bead size.
        let size = rng.range(0.72, 1.30);
        let mut lowest = f32::INFINITY;
        for axis in OCTA_AXES.iter() {
            // Jitter the axis vertex, renormalise: an octahedron that is no
            // longer symmetric — a chip of rock rather than a crystal.
            let jittered = Vec3::new(
                axis[0] + rng.range(-0.28, 0.28),
                axis[1] + rng.range(-0.28, 0.28),
                axis[2] + rng.range(-0.28, 0.28),
            )
            .normalize();
            let offset = jittered * (GRAIN_RADIUS * size);
            offsets.push([offset.x, offset.y, offset.z]);
            normals.push([jittered.x, jittered.y, jittered.z]);
            lowest = lowest.min(offset.y);
        }
        rest.push(-lowest);

        let brightness = rng.range(0.82, 1.16);
        let warm = rng.range(-0.05, 0.05);
        tints.push([
            brightness * (1.0 + warm),
            brightness,
            brightness * (1.0 - warm),
            1.0,
        ]);
    }

    GrainProfile {
        offsets,
        normals,
        rest,
        tints,
    }
}

/// Top-down orthographic, physically lit: a dark aluminium plate with sand
/// piling on the nodes. The camera scale preserves the design's pixel mapping,
/// so the figure stays as legible as the old flat view.
pub fn setup_granular_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn((
        Camera3d::default(),
        crate::render::cinematic(),
        // A generated studio environment so the brushed metal has something to
        // reflect and the scene gets a soft fill.
        EnvironmentMapLight::hemispherical_gradient(
            &mut images,
            Color::srgb(0.55, 0.58, 0.66),
            Color::srgb(0.10, 0.11, 0.15),
            Color::srgb(0.02, 0.02, 0.03),
        ),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            // One world unit == SCALE pixels (`SCALE` is independent of the
            // viewport size, so this is the same mapping the 2D view used).
            scale: 1.0 / SCALE,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            is_active: false,
            clear_color: ClearColorConfig::Custom(STAGE_BG),
            ..default()
        },
        // Straight down; up = -Z keeps +X to the right on screen.
        Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z),
        GranularCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // The plate: a thin aluminium slab, centred so its top face is at y = 0.
    // A dark, slightly-metallic plate: real Chladni plates read as bright sand
    // on a dark ground, and a low albedo keeps the diffuse response from the
    // key light subdued so the grains — the figure — stay the brightest thing.
    let plate_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.055, 0.06, 0.075),
        metallic: 0.5,
        perceptual_roughness: 0.55,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(PLATE_SIZE, PLATE_THICKNESS, PLATE_SIZE))),
        MeshMaterial3d(plate_mat),
        Transform::from_xyz(0.0, -PLATE_THICKNESS * 0.5, 0.0),
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Sand: one mesh, every grain a tiny irregular chip with smooth normals so
    // the light catches it as a rounded speck rather than a painted dot.
    let profile = grain_profile();

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(COUNT * 6);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(COUNT * 6);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(COUNT * 6);
    let mut indices: Vec<u32> = Vec::with_capacity(COUNT * OCTA_FACES.len() * 3);

    for g in 0..COUNT {
        let base = positions.len() as u32;
        for v in 0..6 {
            let o = profile.offsets[g * 6 + v];
            // Parked offscreen until the first snapshot arrives.
            positions.push([OFFSCREEN + o[0], profile.rest[g] + o[1], o[2]]);
            normals.push(profile.normals[g * 6 + v]);
            colors.push(profile.tints[g]);
        }
        for f in OCTA_FACES.iter() {
            indices.push(base + f[0]);
            indices.push(base + f[1]);
            indices.push(base + f[2]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Warm, matte sand. The per-grain vertex shade scales this carrier, so the
    // material stays one sand colour (`material.color * vertex_color`).
    let grain_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.75, 0.60),
        metallic: 0.0,
        perceptual_roughness: 0.85,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(grain_mat),
        GranularParticles,
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));

    commands.insert_resource(profile);
}

pub fn update_granular_particles(
    sim_state: Res<SimState>,
    ui_state: Res<UiState>,
    time: Res<Time>,
    profile: Res<GrainProfile>,
    query: Query<&Mesh3d, With<GranularParticles>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut display: Local<Option<Vec<[f32; 2]>>>,
) {
    // Only the module on screen is refreshed; a hidden module's stale snapshot
    // would otherwise be re-uploaded every frame for nothing.
    if ui_state.active_module != ActiveModule::Granular {
        return;
    }

    let Some(data) = &sim_state.granular_data else {
        return;
    };

    let Ok(mesh3d) = query.single() else {
        return;
    };

    let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
        return;
    };

    let n = data.len().min(COUNT);
    if n == 0 {
        return;
    }

    // --- Temporal filter ------------------------------------------------------
    // Chase the newest solver snapshot instead of copying it: a framerate
    // independent low-pass (`1 - exp(-rate·dt)`) that turns the discrete
    // snapshots into continuous flow. If the snapshot size changed (a solver
    // rebuild) start clean rather than sliding the old bed into the new one.
    let disp = display.get_or_insert_with(Vec::new);
    if disp.len() != n {
        *disp = data.iter().take(n).map(|g| g.position).collect();
    } else {
        let alpha = (1.0 - (-SMOOTH_RATE * time.delta_secs()).exp()).clamp(0.0, 1.0);
        for (d, g) in disp.iter_mut().zip(data.iter()) {
            d[0] += (g.position[0] - d[0]) * alpha;
            d[1] += (g.position[1] - d[1]) * alpha;
        }
    }

    // --- Reconstruct the pile -------------------------------------------------
    // Density of the *displayed* bed on a coarse grid, blurred so a ridge is a
    // smooth swell rather than a noisy scatter.
    let cell_of = |p: [f32; 2]| -> (usize, usize) {
        let gx = (p[0] / PLATE_SIZE * GRID as f32).clamp(0.0, (GRID - 1) as f32) as usize;
        let gy = (p[1] / PLATE_SIZE * GRID as f32).clamp(0.0, (GRID - 1) as f32) as usize;
        (gx, gy)
    };

    let mut grid = vec![0.0f32; GRID * GRID];
    for p in disp.iter() {
        let (gx, gy) = cell_of(*p);
        grid[gy * GRID + gx] += 1.0;
    }
    let grid = blur(&blur(&grid, GRID), GRID);

    // Normalise against the mean density: sparse plate ≈ 0, a packed nodal line
    // saturates at 1.
    let uniform = n as f32 / (GRID * GRID) as f32;
    let full = (uniform * PILE_DENSITY_REF).max(1e-6);
    let pile: Vec<f32> = grid.iter().map(|&d| (d / full).clamp(0.0, 1.0)).collect();

    // --- Rebuild the mesh -----------------------------------------------------
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n * 6);
    let mut indices: Vec<u32> = Vec::with_capacity(n * OCTA_FACES.len() * 3);

    for (i, p) in disp.iter().enumerate() {
        let (gx, gy) = cell_of(*p);
        let level = pile[gy * GRID + gx];

        // Plate-relative simulation coordinate -> world (centred on the plate).
        // up = -Z on screen, so map the sim's +y upward on screen.
        let cx = p[0] - PLATE_SIZE * 0.5;
        let cz = PLATE_SIZE * 0.5 - p[1];
        let cy = profile.rest[i] + level * PILE_HEIGHT;

        // The pile's slope leans the grain normals, so a ridge catches the key
        // light on one flank and falls into shade on the other: real relief.
        let tilt = pile_tilt(&pile, GRID, gx, gy);
        let shade = PILE_MIN_SHADE + PILE_SHADE_GAIN * level;
        let tint = profile.tints[i];

        let base = positions.len() as u32;
        for v in 0..6 {
            let o = profile.offsets[i * 6 + v];
            positions.push([cx + o[0], cy + o[1], cz + o[2]]);
            let nn = (Vec3::from_array(profile.normals[i * 6 + v]) + tilt).normalize();
            normals.push([nn.x, nn.y, nn.z]);
            colors.push([tint[0] * shade, tint[1] * shade, tint[2] * shade, 1.0]);
        }
        for f in OCTA_FACES.iter() {
            indices.push(base + f[0]);
            indices.push(base + f[1]);
            indices.push(base + f[2]);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

/// Outward normal lean induced by the pile's slope at a grid cell, in world
/// space (capped so a sharp ridge never becomes a hard facing edge).
fn pile_tilt(pile: &[f32], w: usize, gx: usize, gy: usize) -> Vec3 {
    let at = |x: isize, y: isize| -> f32 {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, w as isize - 1) as usize;
        pile[y * w + x]
    };
    // d/screen mapping: +x world == +gx, +z world == -gy, so the z term flips.
    let dx = at(gx as isize + 1, gy as isize) - at(gx as isize - 1, gy as isize);
    let dz = at(gx as isize, gy as isize + 1) - at(gx as isize, gy as isize - 1);
    let t = Vec3::new(-dx, 0.0, dz) * TILT_GAIN;
    let m = t.length();
    if m > MAX_TILT {
        t / m * MAX_TILT
    } else {
        t
    }
}

/// One separable 3×3 `[1,2,1]` smoothing pass over a row-major square field.
/// Conserves the total, so the density normalisation is unaffected.
fn blur(field: &[f32], w: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; field.len()];
    let mut out = vec![0.0f32; field.len()];
    for y in 0..w {
        for x in 0..w {
            let l = field[y * w + x.saturating_sub(1)];
            let c = field[y * w + x];
            let r = field[y * w + (x + 1).min(w - 1)];
            tmp[y * w + x] = (l + 2.0 * c + r) * 0.25;
        }
    }
    for y in 0..w {
        let up = y.saturating_sub(1);
        let dn = (y + 1).min(w - 1);
        for x in 0..w {
            out[y * w + x] = (tmp[up * w + x] + 2.0 * tmp[y * w + x] + tmp[dn * w + x]) * 0.25;
        }
    }
    out
}

pub fn toggle_granular_visibility(
    active: Res<UiState>,
    mut vis_query: Query<&mut Visibility, With<GranularCamera>>,
    mut cam_query: Query<&mut Camera, With<GranularCamera>>,
) {
    let visible = active.active_module == ActiveModule::Granular;
    for mut vis in &mut vis_query {
        *vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut cam in &mut cam_query {
        cam.is_active = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_matches_grain_count_and_is_well_formed() {
        let p = grain_profile();
        assert_eq!(p.offsets.len(), COUNT * 6);
        assert_eq!(p.normals.len(), COUNT * 6);
        assert_eq!(p.rest.len(), COUNT);
        assert_eq!(p.tints.len(), COUNT);
        assert!(p.offsets.iter().all(|o| o.iter().all(|c| c.is_finite())));
        assert!(p.rest.iter().all(|r| r.is_finite() && *r > 0.0));
        assert!(p.tints.iter().all(|t| t[3] == 1.0));
    }

    #[test]
    fn blur_conserves_mass() {
        let mut field = vec![0.0f32; GRID * GRID];
        field[10 * GRID + 20] = 8.0;
        let before: f32 = field.iter().sum();
        let after: f32 = blur(&field, GRID).iter().sum();
        assert!((before - after).abs() < 1e-3);
    }

    #[test]
    fn tilt_is_bounded_and_finite() {
        let mut pile = vec![0.0f32; GRID * GRID];
        pile[10 * GRID + 10] = 1.0;
        for gy in [0usize, 10, GRID - 1] {
            for gx in [0usize, 10, GRID - 1] {
                let t = pile_tilt(&pile, GRID, gx, gy);
                assert!(t.is_finite());
                assert!(t.length() <= MAX_TILT + 1e-5);
            }
        }
    }
}
