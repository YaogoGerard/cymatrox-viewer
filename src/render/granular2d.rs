use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::render::{ACCENT, FRAME, PLATE_BG, STAGE_BG};
use crate::{ActiveModule, SimState, UiState};

const LAYER: RenderLayers = RenderLayers::layer(1);

#[derive(Component)]
pub struct GranularCamera;

#[derive(Component)]
pub struct GranularParticles;

const PLATE_SIZE: f32 = 0.5;
const SCALE: f32 = 1200.0;
/// Half the on-screen plate side, in display pixels (`PLATE_SIZE * SCALE`),
/// central to the module viewport. Shared with the UI so the material caption
/// can be pinned just beside the plate's corner.
pub const PLATE_HALF_PX: f32 = PLATE_SIZE * SCALE * 0.5;
const COUNT: usize = 4096;
const GRAIN_RADIUS: f32 = 1.6;

/// Hairline plate frame, in display pixels.
const FRAME_WIDTH: f32 = 1.5;
/// Radius at which the stage vignette reaches its darkest rim. Matches the
/// mock-up's `farthest-corner` ellipse (~1010 px) so the plate sits in the
/// same pool of light.
const STAGE_RADIUS: f32 = 1000.0;
/// The vignette quad, comfortably larger than any viewport.
const STAGE_EXTENT: f32 = 4000.0;
const STAGE_DIVISIONS: u32 = 24;

/// Top-down, orthographic, unlit: the Chladni figure is a *density* pattern
/// (sand piling on the nodes), so the only thing that matters is crisp
/// contrast between the grains and the plate. The mock-up draws it as flat
/// teal strokes on a near-black stage, and reading the same way is what makes
/// the real nodal lines legible — the previous lit metal slab washed them out.
pub fn setup_granular_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        Camera {
            is_active: false,
            clear_color: ClearColorConfig::Custom(STAGE_BG),
            ..default()
        },
        GranularCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Stage: the mock-up's radial vignette (`#13132a` centre -> `#0B0B14`
    // rim). A vertex-coloured quad, because a camera clear colour is flat.
    commands.spawn((
        Mesh2d(meshes.add(stage_mesh())),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::WHITE))),
        Transform::from_xyz(0.0, 0.0, -2.0),
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Plate: the `#242438` hairline square of the mock-up. It has no fill —
    // the stage shows through — so only the rim is drawn (four thin bars).
    let side = PLATE_SIZE * SCALE;
    let half = side * 0.5;
    let frame_mat = materials.add(ColorMaterial::from_color(FRAME));
    let bars = [
        (
            Vec2::new(side + FRAME_WIDTH, FRAME_WIDTH),
            Vec2::new(0.0, half),
        ),
        (
            Vec2::new(side + FRAME_WIDTH, FRAME_WIDTH),
            Vec2::new(0.0, -half),
        ),
        (
            Vec2::new(FRAME_WIDTH, side + FRAME_WIDTH),
            Vec2::new(-half, 0.0),
        ),
        (
            Vec2::new(FRAME_WIDTH, side + FRAME_WIDTH),
            Vec2::new(half, 0.0),
        ),
    ];
    for (size, pos) in bars {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(size.x, size.y))),
            MeshMaterial2d(frame_mat.clone()),
            Transform::from_xyz(pos.x, pos.y, -1.0),
            GranularCamera,
            Visibility::Hidden,
            LAYER,
        ));
    }

    // Single mesh: each grain = center + 6 rim verts (7 verts, 6 tris).
    let verts_per = 7usize;
    let total_verts = COUNT * verts_per;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(COUNT * 18);

    for _ in 0..COUNT {
        let base = positions.len() as u32;
        positions.push([0.0, 0.0, 0.0]);
        colors.push([1.0, 1.0, 1.0, 1.0]);
        for v in 0..6u32 {
            let angle = std::f32::consts::TAU * v as f32 / 6.0;
            positions.push([GRAIN_RADIUS * angle.cos(), GRAIN_RADIUS * angle.sin(), 0.0]);
            colors.push([1.0, 1.0, 1.0, 1.0]);
        }
        for v in 0..6u32 {
            let next = (v + 1) % 6;
            indices.push(base);
            indices.push(base + 1 + v);
            indices.push(base + 1 + next);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Unlit accent: the grains *are* the teal figure of the design. The
    // per-grain vertex shade scales this carrier, so the material stays one
    // flat accent colour (`material.color * vertex_color`).
    let grain_mat = materials.add(ColorMaterial::from_color(ACCENT));

    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(grain_mat),
        GranularParticles,
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));
}

/// Vertex-coloured quad interpolating the stage vignette in linear space.
fn stage_mesh() -> Mesh {
    let n = STAGE_DIVISIONS + 1;
    let centre = PLATE_BG.to_linear();
    let rim = STAGE_BG.to_linear();
    let mut positions = Vec::with_capacity((n * n) as usize);
    let mut colors = Vec::with_capacity((n * n) as usize);

    for iy in 0..n {
        for ix in 0..n {
            let x = -STAGE_EXTENT * 0.5 + STAGE_EXTENT * ix as f32 / STAGE_DIVISIONS as f32;
            let y = -STAGE_EXTENT * 0.5 + STAGE_EXTENT * iy as f32 / STAGE_DIVISIONS as f32;
            positions.push([x, y, 0.0]);
            let t = ((x * x + y * y).sqrt() / STAGE_RADIUS).clamp(0.0, 1.0);
            colors.push([
                centre.red + (rim.red - centre.red) * t,
                centre.green + (rim.green - centre.green) * t,
                centre.blue + (rim.blue - centre.blue) * t,
                1.0,
            ]);
        }
    }

    let mut indices = Vec::with_capacity((STAGE_DIVISIONS * STAGE_DIVISIONS * 6) as usize);
    for iy in 0..STAGE_DIVISIONS {
        for ix in 0..STAGE_DIVISIONS {
            let a = iy * n + ix;
            let b = a + 1;
            let c = a + n;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn update_granular_particles(
    sim_state: Res<SimState>,
    ui_state: Res<UiState>,
    query: Query<&Mesh2d, With<GranularParticles>>,
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

    let Ok(mesh2d) = query.single() else {
        return;
    };

    let Some(mut mesh) = meshes.get_mut(&mesh2d.0) else {
        return;
    };

    let verts_per = 7usize;
    let total_verts = COUNT * verts_per;
    let mut all_pos: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut all_col: Vec<[f32; 4]> = Vec::with_capacity(total_verts);

    for i in 0..COUNT {
        let (cx, cy, col) = if let Some(grain) = data.get(i) {
            // Plate-relative simulation coordinate -> display pixels.
            let [x, y] = grain.position;
            let cx = (x - PLATE_SIZE * 0.5) * SCALE;
            let cy = (y - PLATE_SIZE * 0.5) * SCALE;

            // A small deterministic shade jitter per grain keeps the pile from
            // reading as flat painted pixels without breaking the figure.
            let jitter = ((i as f32 * 12.9898).sin() * 43758.5).fract().abs();
            let shade = 0.70 + 0.30 * jitter;
            (cx, cy, [shade, shade, shade, 1.0])
        } else {
            // Park unused grains far off-screen until the simulation fills in.
            (0.0, -9999.0, [0.0, 0.0, 0.0, 0.0])
        };

        all_pos.push([cx, cy, 0.0]);
        all_col.push(col);
        for v in 0..6u32 {
            let angle = std::f32::consts::TAU * v as f32 / 6.0;
            all_pos.push([
                cx + GRAIN_RADIUS * angle.cos(),
                cy + GRAIN_RADIUS * angle.sin(),
                0.0,
            ]);
            all_col.push(col);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, all_pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, all_col);
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
