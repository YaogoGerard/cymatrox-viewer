use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::SimState;

const LAYER: RenderLayers = RenderLayers::layer(1);

#[derive(Component)]
pub struct GranularCamera;

#[derive(Component)]
pub struct GranularParticles;

const PLATE_SIZE: f32 = 0.5;
const SCALE: f32 = 1200.0;
const COUNT: usize = 4096;
const GRAIN_RADIUS: f32 = 1.35;

pub fn setup_granular_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        GranularCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Plate background
    let plate_w = PLATE_SIZE * SCALE;
    let plate_h = PLATE_SIZE * SCALE;
    let plate_mesh = meshes.add(Rectangle::new(plate_w, plate_h));
    let plate_mat = materials.add(ColorMaterial::from_color(Color::srgb(0.035, 0.06, 0.09)));
    commands.spawn((
        Mesh2d(plate_mesh),
        MeshMaterial2d(plate_mat),
        Transform::from_xyz(0.0, 0.0, -1.0),
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Single mesh: each grain = center + 6 rim verts (7 verts, 6 tris)
    let verts_per = 7u32;
    let total_verts = COUNT * verts_per as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(COUNT * 18);

    for i in 0..COUNT {
        let base = (i * verts_per as usize) as u32;
        positions.push([0.0, 0.0, 0.0]);
        colors.push([0.8, 0.85, 0.9, 1.0]);
        for v in 0..6u32 {
            let angle = std::f32::consts::TAU * v as f32 / 6.0;
            positions.push([GRAIN_RADIUS * angle.cos(), GRAIN_RADIUS * angle.sin(), 0.0]);
            colors.push([0.8, 0.85, 0.9, 1.0]);
        }
        for v in 0..6u32 {
            let next = (v + 1) % 6;
            indices.push(base);
            indices.push(base + 1 + v);
            indices.push(base + 1 + next);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::WHITE))),
        GranularParticles,
        GranularCamera,
        Visibility::Hidden,
        LAYER,
    ));
}

pub fn update_granular_particles(
    sim_state: Res<SimState>,
    mut query: Query<&Mesh2d, With<GranularParticles>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(data) = &sim_state.granular_data else {
        return;
    };

    let Ok(mesh2d) = query.single_mut() else {
        return;
    };

    let Some(mut mesh) = meshes.get_mut(&mesh2d.0) else {
        return;
    };

    let verts_per = 7u32;
    let mut all_pos: Vec<[f32; 3]> = Vec::with_capacity(COUNT * verts_per as usize);
    let mut all_col: Vec<[f32; 4]> = Vec::with_capacity(COUNT * verts_per as usize);

    for i in 0..COUNT.min(data.len()) {
        let d = &data[i];
        // Positions are in [0, plate_size]. Map to [-plate_size/2 * SCALE, +plate_size/2 * SCALE]
        let cx = (d.position[0] - PLATE_SIZE * 0.5) * SCALE;
        let cy = (d.position[1] - PLATE_SIZE * 0.5) * SCALE;

        let speed = (d.velocity[0] * d.velocity[0] + d.velocity[1] * d.velocity[1]).sqrt();
        let t = (speed * 500.0).clamp(0.0, 1.0);
        let r = 0.9 * t + 0.1;
        let g = 0.3 * (1.0 - t) + 0.1;
        let b = 1.0 - t * 0.6;

        all_pos.push([cx, cy, 0.0]);
        all_col.push([r, g, b, 1.0]);
        for v in 0..6u32 {
            let angle = std::f32::consts::TAU * v as f32 / 6.0;
            all_pos.push([
                cx + GRAIN_RADIUS * angle.cos(),
                cy + GRAIN_RADIUS * angle.sin(),
                0.0,
            ]);
            all_col.push([r, g, b, 1.0]);
        }
    }

    for _ in COUNT.min(data.len())..COUNT {
        for _ in 0..verts_per {
            all_pos.push([0.0, -9999.0, 0.0]);
            all_col.push([0.0, 0.0, 0.0, 0.0]);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, all_pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, all_col);
}

pub fn toggle_granular_visibility(
    active: Res<crate::UiState>,
    mut vis_query: Query<&mut Visibility, With<GranularCamera>>,
    mut cam_query: Query<&mut Camera, With<GranularCamera>>,
) {
    let visible = active.active_module == crate::ActiveModule::Granular;
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
