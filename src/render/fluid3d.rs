use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::SimState;

const LAYER: RenderLayers = RenderLayers::layer(2);

#[derive(Component)]
pub struct FluidCamera;

#[derive(Component)]
pub struct FluidHeightmap;

#[derive(Component)]
pub struct FluidVisual;

/// Number of vertices per side of the water surface grid.
const GRID_N: usize = 96;
/// Radius of the circular dish (world units). The Faraday pattern lives here.
const PLATE_R: f32 = 3.0;
/// Fixed display scale: one metre of simulated height becomes this many
/// world units. Unlike normalization, this keeps amplitudes comparable over
/// time and between experiments.
const DISPLAY_HEIGHT_SCALE: f32 = 120.0;
/// Fixed colour reference corresponding to a 3 mm wave.
const COLOUR_REFERENCE_HEIGHT_M: f32 = 0.003;
/// Prevent a numerically unstable frame from turning the water into a giant
/// solid volume. The physical values remain unchanged in the simulation.
const MAX_DISPLAY_HEIGHT: f32 = 0.22;
/// Layout grid resolution used to sample the Faraday field.
const GRID_OUT: usize = 64;

/// Time-smoothed display state so the wave field glides continuously instead
/// of snapping when the (blow-up prone) raw amplitude rescales each frame.
pub struct FluidDisplayState {
    field: Vec<f32>,
}

pub fn setup_fluid_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Perspective camera slightly raised (≈38° above horizontal) so the
    // surface relief and raking light read clearly, while still revealing the
    // radial symmetry of the pattern. Left = press, right = orbit, wheel = zoom.
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        Transform::from_xyz(0.0, 4.5, 7.5).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera {
            focus: Vec3::ZERO,
            button_orbit: MouseButton::Left,
            button_pan: MouseButton::Right,
            button_zoom: Some(MouseButton::Middle),
            enabled: false,
            ..default()
        },
        FluidCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // Raking key light: near-horizontal directional light so each wave crest
    // casts a sharp shadow into the neighboring trough (the classic cymatics
    // photographic look). Reddish-gold tint for warm highlights.
    commands.spawn((
        DirectionalLight {
            // Bright, near-grazing key light: each crest casts a hard shadow
            // into the neighbouring trough. Muted gold tint for warm sheens.
            illuminance: 16000.0,
            color: Color::srgb(1.0, 0.94, 0.82),
            shadow_maps_enabled: true,
            ..default()
        },
        // Almost horizontal: 8° above the plate plane, sweeping from the side.
        Transform::from_rotation(Quat::from_rotation_x(-0.14) * Quat::from_rotation_y(-1.2)),
        LAYER,
    ));

    // The water surface: a flat circular dish (a filled square whose corners
    // fall outside the plate circle and lie flat at dish level).
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    let vertex_count = GRID_N * GRID_N;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_count);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(vertex_count);
    let mut indices: Vec<u32> = Vec::with_capacity((GRID_N - 1) * (GRID_N - 1) * 6);

    let half = PLATE_R * 1.12;
    let step = (2.0 * half) / (GRID_N as f32 - 1.0);
    for iy in 0..GRID_N {
        for ix in 0..GRID_N {
            let x = -half + ix as f32 * step;
            let y = -half + iy as f32 * step;
            positions.push([x, 0.0, y]);
            uvs.push([
                ix as f32 / (GRID_N - 1) as f32,
                iy as f32 / (GRID_N - 1) as f32,
            ]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push([0.02, 0.05, 0.10, 1.0]);
        }
    }
    for iy in 0..(GRID_N - 1) as u32 {
        for ix in 0..(GRID_N - 1) as u32 {
            let tl = iy * GRID_N as u32 + ix;
            let tr = tl + 1;
            let bl = (iy + 1) * GRID_N as u32 + ix;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Surface color comes from per-vertex colors (crests cyan, troughs deep
    // blue) so the pattern is readable whatever the lighting. White base lets
    // those vertex colors show through; moderate roughness keeps water sheen.
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.025, 0.16, 0.28),
        perceptual_roughness: 0.16,
        metallic: 0.0,
        emissive: LinearRgba::rgb(0.0, 0.015, 0.025),
        specular_transmission: 0.0,
        cull_mode: None,
        ..default()
    });

    let dish_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.025, 0.045),
        metallic: 0.55,
        perceptual_roughness: 0.28,
        ..default()
    });
    commands.spawn((
        Mesh3d(
            meshes.add(
                Cylinder::new(PLATE_R * 1.04, 0.10)
                    .mesh()
                    .resolution(96)
                    .build(),
            ),
        ),
        MeshMaterial3d(dish_mat),
        Transform::from_xyz(0.0, -0.10, 0.0),
        FluidCamera,
        FluidVisual,
        Visibility::Hidden,
        LAYER,
    ));

    let rim_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.18, 0.24),
        metallic: 0.8,
        perceptual_roughness: 0.2,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(PLATE_R * 0.98, PLATE_R * 1.04).mesh().build())),
        MeshMaterial3d(rim_mat),
        Transform::from_xyz(0.0, 0.015, 0.0),
        FluidCamera,
        FluidVisual,
        Visibility::Hidden,
        LAYER,
    ));

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(mat),
        Transform::default(),
        FluidHeightmap,
        FluidCamera,
        FluidVisual,
        Visibility::Hidden,
        LAYER,
    ));
}

pub fn update_fluid_mesh(
    sim_state: Res<SimState>,
    query: Query<&Mesh3d, With<FluidHeightmap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: Local<Option<FluidDisplayState>>,
    mut last_log: Local<Option<std::time::Instant>>,
) {
    let Some(data) = &sim_state.fluid_data else {
        return;
    };

    let Ok(mesh3d) = query.single() else {
        return;
    };
    let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
        return;
    };

    {
        let now = std::time::Instant::now();
        let do_log = last_log
            .as_ref()
            .map(|t| now.duration_since(*t) >= std::time::Duration::from_secs(1))
            .unwrap_or(true);
        if do_log {
            let max_h = data.iter().map(|d| d.height.abs()).fold(0.0f32, f32::max);
            eprintln!(
                "[fluid3d] update: {} nodes, max |h|={:.3e} m",
                data.len(),
                max_h
            );
            *last_log = Some(now);
        }
    }

    let node_count = data.len().min(GRID_OUT * GRID_OUT);
    // Keep the physical amplitude. Only non-finite samples are discarded;
    // there is no per-frame normalization or hidden gain.
    let mut target: Vec<f32> = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let height = data[i].height;
        target.push(if height.is_finite() {
            (height * DISPLAY_HEIGHT_SCALE).clamp(-MAX_DISPLAY_HEIGHT, MAX_DISPLAY_HEIGHT)
        } else {
            0.0
        });
    }

    // Time-smooth into the display field.
    let mut hlin: Vec<f32> = Vec::with_capacity(node_count);
    if let Some(st) = state.as_mut() {
        if st.field.len() == node_count {
            for i in 0..node_count {
                hlin.push(st.field[i] + 0.8 * (target[i] - st.field[i]));
            }
            st.field = hlin.clone();
        } else {
            hlin = target.clone();
            *st = FluidDisplayState {
                field: target.clone(),
            };
        }
    } else {
        hlin = target.clone();
        *state = Some(FluidDisplayState {
            field: target.clone(),
        });
    }

    // Spatial smoothing: a light 3x3 convolution removes the pure numerical
    // noise (noise_amplitude) WITHOUT erasing the Faraday dome networks, which
    // the heavier 5x5 used to smear into an unreadable mush.
    let hlin = smooth_field(&hlin, GRID_OUT);

    // Press deformation is disabled: the left mouse button is now used to
    // Map each grid vertex to a plate position, sample the field, apply height.
    let half = PLATE_R * 1.12;
    let step = (2.0 * half) / (GRID_N as f32 - 1.0);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(GRID_N * GRID_N);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(GRID_N * GRID_N);
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for iy in 0..GRID_N {
        for ix in 0..GRID_N {
            let x = -half + ix as f32 * step;
            let y = -half + iy as f32 * step;
            let r = (x * x + y * y).sqrt();
            let mut h = 0.0f32;
            if r <= PLATE_R {
                let u = (x / PLATE_R).clamp(-1.0, 1.0);
                let v = (y / PLATE_R).clamp(-1.0, 1.0);
                let raw = sample_bilinear(u, v, &hlin);
                h = raw;
                // Slight dish-rim falloff so the surface meets the plate edge.
                let rim = 1.0 - (r / PLATE_R).max(0.0).powi(2);
                h *= 0.35 + 0.65 * rim;
            }
            let z = h;
            min_z = min_z.min(z);
            max_z = max_z.max(z);

            positions.push([x, z, y]);

            // Vertex color from height: crests bright cyan, troughs deep blue.
            // Guarantees the pattern is readable whatever the lighting.
            let amp = (h / (COLOUR_REFERENCE_HEIGHT_M * DISPLAY_HEIGHT_SCALE)).clamp(-1.0, 1.0);
            let deep = Vec3::new(0.01, 0.06, 0.16);
            let crest = Vec3::new(0.25, 0.82, 0.95);
            let col = if amp >= 0.0 {
                deep.lerp(crest, amp)
            } else {
                deep.lerp(Vec3::new(0.0, 0.0, 0.05), -amp)
            };
            colors.push([col.x, col.y, col.z, 1.0]);
        }
    }
    if min_z.is_finite() && max_z.is_finite() {
        eprintln!(
            "[fluid3d] surface extent: y in [{:.3}, {:.3}] (range={:.3})",
            min_z,
            max_z,
            max_z - min_z
        );
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    // Recompute smoothed normals from the displaced grid for correct relief.
    let normals = compute_normals(&mesh_positions(&mesh), GRID_N, half);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
}

fn mesh_positions(mesh: &Mesh) -> Vec<[f32; 3]> {
    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|p| p.as_float3())
        .map(|v| v.to_vec())
        .unwrap_or_default()
}

/// Recompute surface normals by finite differences over the grid so the
/// raking light shades each crest/trough correctly.
fn compute_normals(positions: &[[f32; 3]], n: usize, half: f32) -> Vec<[f32; 3]> {
    let step = (2.0 * half) / (n as f32 - 1.0);
    let h = |cx: i32, cy: i32| -> f32 {
        let nx = cx.clamp(0, n as i32 - 1) as usize;
        let ny = cy.clamp(0, n as i32 - 1) as usize;
        positions[ny * n + nx][1]
    };
    let mut normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    for iy in 0..n {
        for ix in 0..n {
            let gx = (h(ix as i32 + 1, iy as i32) - h(ix as i32 - 1, iy as i32)) / (2.0 * step);
            let gy = (h(ix as i32, iy as i32 + 1) - h(ix as i32, iy as i32 - 1)) / (2.0 * step);
            let mut n3 = Vec3::new(-gx, 1.0, -gy);
            if n3.length_squared() < 1e-12 {
                n3 = Vec3::Z;
            }
            n3 = n3.normalize();
            normals[iy * n + ix] = [n3.x, n3.y, n3.z];
        }
    }
    normals
}

/// Bilinear sample of the (row-major, 2D) field at an in-plate coordinate
/// (u, v) in [-1, 1].
fn sample_bilinear(u: f32, v: f32, field: &[f32]) -> f32 {
    let g = (GRID_OUT as f32 - 1.0).max(1.0);
    let fx = (u + 1.0) * 0.5 * g;
    let fy = (v + 1.0) * 0.5 * g;
    let x0 = fx.floor().max(0.0) as usize;
    let y0 = fy.floor().max(0.0) as usize;
    let x1 = (x0 + 1).min(GRID_OUT - 1);
    let y1 = (y0 + 1).min(GRID_OUT - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let at = |x: usize, y: usize| field.get(y * GRID_OUT + x).copied().unwrap_or(0.0);
    let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
    let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
    top * (1.0 - ty) + bot * ty
}

/// Spatial 3x3 Gaussian-ish convolution that removes pure numerical grit while
/// preserving the Faraday dome/network structure (a 5x5 was too aggressive and
/// smeared the pattern into an unreadable mush).
fn smooth_field(field: &[f32], w: usize) -> Vec<f32> {
    let h = w;
    let mut out = vec![0.0f32; field.len()];
    const K: usize = 1;
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0f32;
            let mut wsum = 0.0f32;
            for dy in -(K as i32)..=(K as i32) {
                for dx in -(K as i32)..=(K as i32) {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let wt = 1.0 / ((dx * dx + dy * dy) as f32 + 1.0);
                    sum += field[(ny as usize) * w + nx as usize] * wt;
                    wsum += wt;
                }
            }
            out[y * w + x] = sum / wsum;
        }
    }
    out
}

pub fn toggle_fluid_visibility(
    ui_state: Res<crate::UiState>,
    mut vis_query: Query<&mut Visibility, With<FluidVisual>>,
    mut cam_query: Query<&mut Camera, With<FluidCamera>>,
) {
    let visible = ui_state.active_module == crate::ActiveModule::Fluid;
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
