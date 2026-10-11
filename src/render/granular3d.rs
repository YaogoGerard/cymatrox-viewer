//! Granular (Chladni) module, rendered as a physically-lit 3D scene.
//!
//! Earlier this module was a flat, unlit 2D view: crisp, but stylised. It is now
//! a real aluminium plate with sand grains, lit by the shared key light and a
//! generated environment. To keep the Chladni figure just as legible as before —
//! dark plate, bright grains piling on the nodal lines — the camera stays a
//! *top-down orthographic* view and is scaled so one world unit is exactly
//! `SCALE` pixels, matching the old 2D mapping (so `PLATE_HALF_PX`, used by the
//! UI caption, remains correct).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::render::STAGE_BG;
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
const COUNT: usize = 4096;

/// Plate thickness in world units (a thin slab, not a sheet of paper).
const PLATE_THICKNESS: f32 = 0.012;
/// Grain radius in display pixels, converted to world units through [`SCALE`].
const GRAIN_RADIUS_PX: f32 = 1.6;
const GRAIN_RADIUS: f32 = GRAIN_RADIUS_PX / SCALE;

/// Grain rest height: an octahedron of radius `r` centred at `y = r` just
/// touches the plate top (drawn at `y = 0`).
const GRAIN_REST_Y: f32 = GRAIN_RADIUS;

/// Parking offset for grains that have no simulation data yet (kept far outside
/// the orthographic frustum instead of drawing a clump at the origin).
const OFFSCREEN: f32 = -1000.0;

/// Unit octahedron used as the sand grain. Six vertices, smooth (radial)
/// normals, eight faces — enough to catch the key light as a rounded speck.
const OCTA_VERTS: [[f32; 3]; 6] = [
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

    // Sand: one mesh, each grain a tiny octahedron with smooth normals so the
    // light catches it as a rounded speck rather than a painted dot.
    let total_verts = COUNT * OCTA_VERTS.len();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(COUNT * OCTA_FACES.len() * 3);

    for _ in 0..COUNT {
        let base = positions.len() as u32;
        for v in OCTA_VERTS.iter() {
            positions.push([
                OFFSCREEN + v[0] * GRAIN_RADIUS,
                GRAIN_REST_Y + v[1] * GRAIN_RADIUS,
                v[2] * GRAIN_RADIUS,
            ]);
            normals.push(*v);
            colors.push([1.0, 1.0, 1.0, 1.0]);
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
}

pub fn update_granular_particles(
    sim_state: Res<SimState>,
    ui_state: Res<UiState>,
    query: Query<&Mesh3d, With<GranularParticles>>,
    mut meshes: ResMut<Assets<Mesh>>,
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

    let n = data.len();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * OCTA_VERTS.len());
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * OCTA_VERTS.len());
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n * OCTA_VERTS.len());
    let mut indices: Vec<u32> = Vec::with_capacity(n * OCTA_FACES.len() * 3);

    for (i, grain) in data.iter().enumerate() {
        // Plate-relative simulation coordinate -> world (centred on the plate).
        let [sx, sy] = grain.position;
        let cx = sx - PLATE_SIZE * 0.5;
        // up = -Z on screen, so map the sim's +y upward on screen.
        let cz = PLATE_SIZE * 0.5 - sy;

        // A small deterministic shade jitter per grain keeps the pile from
        // reading as flatly painted without breaking the figure.
        let jitter = ((i as f32 * 12.9898).sin() * 43758.5).fract().abs();
        let shade = 0.75 + 0.25 * jitter;

        let base = positions.len() as u32;
        for v in OCTA_VERTS.iter() {
            positions.push([
                cx + v[0] * GRAIN_RADIUS,
                GRAIN_REST_Y + v[1] * GRAIN_RADIUS,
                cz + v[2] * GRAIN_RADIUS,
            ]);
            normals.push(*v);
            colors.push([shade, shade, shade, 1.0]);
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
