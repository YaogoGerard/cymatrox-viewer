use bevy::camera::visibility::RenderLayers;
use bevy::log::{debug, warn};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy_panorbit_camera::PanOrbitCamera;

use crate::{ActiveModule, SimState, UiState};

const LAYER: RenderLayers = RenderLayers::layer(3);

/// World-space half-size of the visual volume (metres->display units scaled).
const WORLD_HALF: f32 = 1.3;

#[derive(Component)]
pub struct AcousticCamera;

#[derive(Component)]
pub struct AcousticVolume;

/// Marker applied to every visible entity in the acoustic scene so it can be
/// shown/hidden together when the module is selected.
#[derive(Component)]
pub struct AcousticVisual;

#[derive(Component)]
pub struct AcousticParticle;

fn grid_dim(len: usize) -> usize {
    (((len as f64).cbrt()).round() as usize).max(1)
}

/// Smooth, low-opacity diverging colormap. p>0 -> red, p<0 -> blue, and
/// values close to zero are discarded so the volume does not become a stack
/// of opaque slabs. Returns [r,g,b,a].
fn pressure_color(p: f32, pmax: f32) -> [f32; 4] {
    let t = (p / pmax.max(1e-9)).clamp(-1.0, 1.0);
    let magnitude = t.abs();
    const CUTOFF: f32 = 0.16;
    if magnitude <= CUTOFF {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let a = ((magnitude - CUTOFF) / (1.0 - CUTOFF)).powf(0.7);
    if p >= 0.0 {
        // Red compression lobe, with enough transparency to see through it.
        [0.75 * a, 0.08 * a, 0.04 * a, 0.025 + 0.10 * a]
    } else {
        // Blue rarefaction lobe.
        [0.04 * a, 0.10 * a, 0.65 * a, 0.025 + 0.10 * a]
    }
}

/// Trilinear sampling of the input grid (row-major, x fastest).
/// `f` coordinates are in [0, n-1].
fn sample_trilinear(
    data: &[cymatrox::acoustic::AcousticPressureNode],
    n: usize,
    x: f32,
    y: f32,
    z: f32,
) -> f32 {
    let x0 = (x.floor() as i32).clamp(0, n as i32 - 1) as usize;
    let y0 = (y.floor() as i32).clamp(0, n as i32 - 1) as usize;
    let z0 = (z.floor() as i32).clamp(0, n as i32 - 1) as usize;
    let x1 = (x0 + 1).min(n - 1);
    let y1 = (y0 + 1).min(n - 1);
    let z1 = (z0 + 1).min(n - 1);
    let tx = (x - x0 as f32).clamp(0.0, 1.0);
    let ty = (y - y0 as f32).clamp(0.0, 1.0);
    let tz = (z - z0 as f32).clamp(0.0, 1.0);
    let at = |x: usize, y: usize, z: usize| {
        data.get(x + n * (y + n * z))
            .map(|d| d.pressure_pa)
            .unwrap_or(0.0)
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    // Interpolate along x on each of the 4 edges of the block, then y, then z.
    let c00 = lerp(at(x0, y0, z0), at(x1, y0, z0), tx);
    let c10 = lerp(at(x0, y1, z0), at(x1, y1, z0), tx);
    let c01 = lerp(at(x0, y0, z1), at(x1, y0, z1), tx);
    let c11 = lerp(at(x0, y1, z1), at(x1, y1, z1), tx);
    let c0 = lerp(c00, c10, ty);
    let c1 = lerp(c01, c11, ty);
    lerp(c0, c1, tz)
}

/// Fixed colour reference (pascals). The volume used to be normalised by the
/// per-frame maximum pressure, which pinned the display to full brightness
/// every frame and made the drive amplitude invisible. A constant reference
/// means brightness now tracks the actual field: stronger drive = brighter,
/// weaker field = dimmer, and the two can finally be compared.
const PRESSURE_REFERENCE_PA: f32 = 1.0;

/// Build a sparse voxel cloud. Disconnected, softly transparent cells make
/// pressure variations read as a gas density instead of stacked cut planes.
fn build_volume_mesh(data: &[cymatrox::acoustic::AcousticPressureNode], n: usize) -> Mesh {
    let half = WORLD_HALF;
    let pmax = PRESSURE_REFERENCE_PA;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let cells = n.saturating_sub(1).max(1);
    let spacing = (2.0 * half) / cells as f32;
    let cell_size = spacing * 0.68;
    let faces = [
        (
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ),
        (
            [1.0, -1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [0.0, 0.0, -1.0],
        ),
        (
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ),
        (
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [0.0, -1.0, 0.0],
        ),
        (
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 0.0, 0.0],
        ),
        (
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 0.0, 0.0],
        ),
    ];
    for ix in 0..cells {
        for iy in 0..cells {
            for iz in 0..cells {
                let sx = (ix as f32 + 0.5) / cells as f32 * (n as f32 - 1.0);
                let sy = (iy as f32 + 0.5) / cells as f32 * (n as f32 - 1.0);
                let sz = (iz as f32 + 0.5) / cells as f32 * (n as f32 - 1.0);
                let pressure = sample_trilinear(data, n, sx, sy, sz);
                let color = pressure_color(pressure, pmax);
                if color[3] == 0.0 {
                    continue;
                }
                let center = [
                    -half + (ix as f32 + 0.5) * spacing,
                    -half + (iy as f32 + 0.5) * spacing,
                    -half + (iz as f32 + 0.5) * spacing,
                ];
                for (a, b, c, d, normal) in faces {
                    let base = positions.len() as u32;
                    for corner in [a, b, c, d] {
                        positions.push([
                            center[0] + corner[0] * cell_size * 0.5,
                            center[1] + corner[1] * cell_size * 0.5,
                            center[2] + corner[2] * cell_size * 0.5,
                        ]);
                        colors.push(color);
                        normals.push(normal);
                    }
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Build a metallic disc (piezoelectric transducer face) facing the +X or -X
/// direction. Returns the mesh.
fn build_disc_mesh(radius: f32, thickness: f32) -> Mesh {
    Cylinder::new(radius, thickness)
        .mesh()
        .resolution(48)
        .build()
}

/// Build a glowing torus ring (activty rim) around a transducer face.
fn build_rim_mesh(radius: f32, tube: f32) -> Mesh {
    Torus::new(radius - tube, radius).mesh().build()
}

pub fn setup_acoustic_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // --- Camera: 3/4 perspective orbit, dark clear color, module viewport ---
    commands.spawn((
        Camera3d::default(),
        // Cheap generated studio environment so the machined transducers read
        // as metal (they need something to reflect) and the scene gets a soft
        // ambient fill. Built once, no per-frame cost.
        EnvironmentMapLight::hemispherical_gradient(
            &mut images,
            Color::srgb(0.18, 0.20, 0.26),
            Color::srgb(0.055, 0.065, 0.085),
            Color::srgb(0.014, 0.016, 0.022),
        ),
        Camera {
            is_active: false,
            clear_color: ClearColorConfig::Custom(crate::render::STAGE_BG),
            ..default()
        },
        Transform::from_xyz(2.6, 1.7, 2.6).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera {
            focus: Vec3::ZERO,
            yaw: Some(-0.75),
            pitch: Some(0.5),
            button_orbit: MouseButton::Left,
            button_pan: MouseButton::Right,
            button_zoom: Some(MouseButton::Middle),
            enabled: false,
            ..default()
        },
        AcousticCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // --- Volumetric field. Alpha blending keeps overlapping slices legible
    // instead of accumulating into opaque red/blue walls.
    let field_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: bevy_material::AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(build_volume_mesh(&[], 16))),
        MeshMaterial3d(field_mat),
        AcousticVolume,
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));

    // A faint acrylic chamber gives the pressure field a physical enclosure
    // instead of leaving the slices floating in empty space. Low roughness so
    // its edges catch a sharp glass-like highlight without paying for real
    // refraction.
    let chamber_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.72, 0.82, 0.90, 0.045),
        alpha_mode: bevy_material::AlphaMode::Blend,
        metallic: 0.0,
        perceptual_roughness: 0.04,
        reflectance: 0.6,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            WORLD_HALF * 2.0,
            WORLD_HALF * 2.0,
            WORLD_HALF * 2.0,
        ))),
        MeshMaterial3d(chamber_mat),
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));

    // --- Transducers: two flat metallic discs facing each other along X ---
    let disc_radius = 1.35;
    let disc_thickness = 0.09;
    let trans_pos = WORLD_HALF + 0.12;
    let metallic = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.64, 0.68),
        perceptual_roughness: 0.25,
        metallic: 0.9,
        ..default()
    });

    // Low-side (active) transducer at -X: shaft along X via rot Y by +90°.
    commands.spawn((
        Mesh3d(meshes.add(build_disc_mesh(disc_radius, disc_thickness))),
        MeshMaterial3d(metallic.clone()),
        Transform::from_xyz(-trans_pos, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));
    // High-side (receiving) transducer at +X.
    commands.spawn((
        Mesh3d(meshes.add(build_disc_mesh(disc_radius, disc_thickness))),
        MeshMaterial3d(metallic),
        Transform::from_xyz(trans_pos, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));

    // --- Machined clamp rings on the inner face of each transducer ---
    let rim_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.58, 0.63),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        // Barely-there teal so the driven transducer still reads as "active".
        emissive: LinearRgba::rgb(0.0, 0.04, 0.03),
        cull_mode: None,
        ..default()
    });
    let rim_radius = 1.2;
    let rim_tube = 0.02;
    let rim_off = 0.02;
    commands.spawn((
        Mesh3d(meshes.add(build_rim_mesh(rim_radius, rim_tube))),
        MeshMaterial3d(rim_mat.clone()),
        Transform::from_xyz(-trans_pos + rim_off, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));
    commands.spawn((
        Mesh3d(meshes.add(build_rim_mesh(rim_radius, rim_tube))),
        MeshMaterial3d(rim_mat),
        Transform::from_xyz(trans_pos - rim_off, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));

    // --- Levitating object: a small glossy bead held at the nodal point in
    //     the centre. Polished and highly specular so it reads as a real
    //     trapped particle rather than a glowing UI dot.
    let sphere_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.94, 0.98),
        perceptual_roughness: 0.06,
        metallic: 0.0,
        reflectance: 0.55,
        clearcoat: 0.6,
        clearcoat_perceptual_roughness: 0.1,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.14).mesh().uv(32, 18))),
        MeshMaterial3d(sphere_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        AcousticParticle,
        AcousticVisual,
        Visibility::Hidden,
        LAYER,
    ));
}

/// Frame-to-frame state the acoustic view keeps between runs (grouped so the
/// system does not carry a long tail of separate `Local` parameters).
#[derive(Default)]
pub(crate) struct AcousticFrameState {
    last_log: Option<std::time::Instant>,
    last_rebuild: Option<std::time::Instant>,
    particle_velocity: Vec3,
}

pub fn update_acoustic_points(
    sim_state: Res<SimState>,
    ui_state: Res<UiState>,
    mut query: Query<&Mesh3d, With<AcousticVolume>>,
    mut particle_query: Query<&mut Transform, With<AcousticParticle>>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut frame: Local<AcousticFrameState>,
) {
    // Only the visible module is refreshed; hidden snapshots are stale.
    if ui_state.active_module != ActiveModule::Acoustic {
        return;
    }

    let Some(data) = &sim_state.acoustic_data else {
        return;
    };

    let Ok(mesh3d) = query.single_mut() else {
        warn!("[acoustic] no AcousticVolume entity found");
        return;
    };

    let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
        warn!("[acoustic] mesh handle invalid");
        return;
    };

    let now = std::time::Instant::now();
    let do_log = frame
        .last_log
        .as_ref()
        .map(|t| now.duration_since(*t) >= std::time::Duration::from_secs(1))
        .unwrap_or(true);
    if do_log {
        let pmax = data
            .iter()
            .map(|d| d.pressure_pa.abs())
            .fold(0.0f32, f32::max);
        debug!(
            "[acoustic] update: {} nodes, |p|max={:.4} Pa",
            data.len(),
            pmax
        );
        frame.last_log = Some(now);
    }

    // Rebuilding the dense stack on every frame is wasteful on the iGPU; cap
    // it to ~20x/s so the volume refreshes but stays smooth.
    let due = frame
        .last_rebuild
        .as_ref()
        .map(|t| now.duration_since(*t) >= std::time::Duration::from_millis(50))
        .unwrap_or(true);
    if due {
        let n = grid_dim(data.len());
        *mesh = build_volume_mesh(data, n);
        frame.last_rebuild = Some(now);
    }

    // Integrate the simulated Gor'kov force for the displayed particle.
    // The solver reports force in newtons; the visual volume is scaled from
    // metres to world units below.
    if let Ok(mut transform) = particle_query.single_mut() {
        let center = n_center(data);
        let force = data
            .get(center)
            .map(|node| Vec3::from_array(node.force))
            .unwrap_or(Vec3::ZERO);
        let mass = 4.0 / 3.0 * std::f32::consts::PI * 0.001_f32.powi(3) * 1000.0;
        let acceleration = force / mass - Vec3::new(0.0, 9.81, 0.0);
        let dt = time.delta_secs().min(0.05);
        frame.particle_velocity = (frame.particle_velocity + acceleration * dt) * (-2.0 * dt).exp();
        transform.translation += frame.particle_velocity * dt * 65.0;
        transform.translation.x = transform.translation.x.clamp(-1.1, 1.1);
        transform.translation.y = transform.translation.y.clamp(-1.1, 1.1);
        transform.translation.z = transform.translation.z.clamp(-1.1, 1.1);
    }
}

fn n_center(data: &[cymatrox::acoustic::AcousticPressureNode]) -> usize {
    let n = grid_dim(data.len());
    let c = n / 2;
    c + n * (c + n * c)
}

pub fn toggle_acoustic_visibility(
    ui_state: Res<crate::UiState>,
    mut cam_query: Query<(&mut Visibility, &mut Camera), With<AcousticCamera>>,
    mut visual_query: Query<&mut Visibility, (With<AcousticVisual>, Without<AcousticCamera>)>,
) {
    let visible = ui_state.active_module == crate::ActiveModule::Acoustic;
    for (mut vis, mut cam) in &mut cam_query {
        *vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        cam.is_active = visible;
    }
    for mut vis in &mut visual_query {
        *vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
