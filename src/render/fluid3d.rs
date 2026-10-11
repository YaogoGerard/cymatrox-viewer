use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::ScalingMode;
use bevy::input::mouse::MouseWheel;
use bevy::log::debug;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::render::{glyph, ACCENT};
use crate::{ActiveModule, UiState};

const LAYER: RenderLayers = RenderLayers::layer(2);

#[derive(Component)]
pub struct FluidCamera;

#[derive(Component)]
pub struct FluidHeightmap;

#[derive(Component)]
pub struct FluidVisual;

/// Radius of the circular water pool (world units). The Faraday pattern lives
/// inside this circle; it is the physical dish of the simulation.
pub const PLATE_R: f32 = 3.0;

/// Physical radius of the dish (m), matching the fluid domain. The analytic
/// glyph's Bessel wavenumbers are `j_{m,n} / PLATE_RADIUS_M`.
const PLATE_RADIUS_M: f32 = 0.025;

/// The "scope" hardware (bezel + graduations) reaches out to this radius. The
/// orthographic fit reserves [`SCOPE_FRACTION`] of the shorter viewport edge for
/// it, so the whole instrument always fits whatever the window aspect.
pub const SCOPE_FIT_R: f32 = 3.6;
pub const SCOPE_FRACTION: f32 = 0.60;

/// Graduated bezel ring drawn around the pool.
const BEZEL_INNER: f32 = 3.06;
const BEZEL_OUTER: f32 = 3.34;
const BEZEL_SEGMENTS: usize = 160;
/// Accent hairline drawn right at the water's edge, echoing the UI accent.
const HAIRLINE_INNER: f32 = 3.010;
const HAIRLINE_OUTER: f32 = 3.026;
/// Number of graduations around the bezel; every `CARDINAL_EVERY`-th is longer.
const N_TICKS: usize = 72;
const CARDINAL_EVERY: usize = 6;

/// Target peak of the displayed relief, in world units. The analytic glyph is
/// normalised per frame so its tallest feature always reaches this height:
/// otherwise the physical amplitude (tens of micrometres) is so far below the
/// dish radius that the surface is optically flat and real lighting has nothing
/// to catch. `MAX_DISPLAY_HEIGHT` still guards the extremes.
const DISPLAY_TARGET_HEIGHT: f32 = 0.32;
/// Floor of the live slope reference (display units) used to autoscale the
/// froth response. A calm pool stays dark instead of amplifying noise.
const MIN_SLOPE_REF: f32 = 0.05;
/// Prevent a numerically unstable frame from turning the water into a giant
/// solid volume (~4 mm of water at the display scale). The physical values
/// remain unchanged in the simulation.
const MAX_DISPLAY_HEIGHT: f32 = 0.5;
/// Layout grid resolution used to sample the Faraday field. The analytic glyph
/// carries several tens of nodal rings at the top of the audio range, so this is
/// much finer than the old solver readback grid.
const GRID_OUT: usize = 256;

/// Water disc tessellation: concentric rings of vertices, uniform angular
/// resolution. A polar mesh gives a clean circular pool edge (a square grid
/// would leave flat corners sticking out around the dish).
const DISC_RINGS: usize = 36;
const DISC_SEGMENTS: usize = 120;

/// Zoom on the scope (mouse wheel). `1.0` is the fitted default framing; larger
/// values move the camera closer. Lives in a resource because both the fit and
/// the zoom systems read it.
#[derive(Resource)]
pub struct FluidZoom(pub f32);

/// Time-smoothed display state so the figure glides continuously when the tone
/// changes instead of snapping between modes. `target` is the analytic glyph for
/// the current `key` (quantised tone + liquid), recomputed only when it changes;
/// `field` chases it each frame.
pub struct FluidDisplayState {
    field: Vec<f32>,
    target: Vec<f32>,
    key: [u32; 4],
}

pub fn setup_fluid_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.insert_resource(FluidZoom(1.0));

    // Top-down orthographic "scope" camera, looking straight down at the pool.
    // The A Cymascope presents the vibrating dish from directly overhead, so
    // there is no perspective or orbit: the circle reads as a perfect ring. The
    // projection's `scale` is refit every frame by `fit_fluid_scope`.
    commands.spawn((
        Camera3d::default(),
        crate::render::cinematic(),
        // A generated studio environment (no asset needed): the water is a
        // near-mirror surface, so it needs something to reflect. Bright at the
        // zenith, falling to a dark nadir — the reflected sky gradient is what
        // makes the pool read as liquid instead of a black disc.
        EnvironmentMapLight::hemispherical_gradient(
            &mut images,
            Color::srgb(0.62, 0.72, 0.88),
            Color::srgb(0.10, 0.14, 0.20),
            Color::srgb(0.012, 0.016, 0.024),
        ),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            // Sensible first-frame value; corrected by `fit_fluid_scope`.
            scale: 0.021,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            is_active: false,
            // Same dark stage as the mock-up, so every module shares the
            // panel's palette instead of each inventing its own background.
            clear_color: ClearColorConfig::Custom(crate::render::STAGE_BG),
            ..default()
        },
        // Straight down; up = -Z keeps +X to the right on screen.
        Transform::from_xyz(0.0, 30.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z),
        FluidCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // A dedicated key for the liquid layer: a low-angle (raking) beam so the
    // wave slopes catch the light and the relief reads through real shading and
    // specular highlights instead of a baked colour. Illuminance is high because
    // the disc is a dark, low-roughness dielectric that mostly reflects.
    commands.spawn((
        DirectionalLight {
            illuminance: 14_000.0,
            color: Color::srgb(0.94, 0.97, 1.0),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.02, 0.55, 0.0)),
        RenderLayers::layer(2),
    ));

    // The water pool: a disc that meets the bezel exactly at `PLATE_R`.
    let (positions, indices) = disc_geometry();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let colors = vec![[0.03, 0.09, 0.16, 1.0]; positions.len()];
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let uvs = vec![[0.0, 0.0]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Water: a dark, low-roughness dielectric. Its albedo (including slope-based
    // froth on the steepest crests) is carried per-vertex, so the material base
    // colour stays white; the *brightness* now comes from a physically-lit
    // surface — the raking key's diffuse + specular on the wave normals plus the
    // reflected environment map, which is exactly the signal a CymaScope records.
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        metallic: 0.0,
        perceptual_roughness: 0.12,
        reflectance: 0.55,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(mat),
        Transform::default(),
        FluidHeightmap,
        FluidVisual,
        Visibility::Hidden,
        LAYER,
    ));

    // The instrument: bezel, graduations and accent hairline. A lit, brushed
    // metal so the plate hardware reads as a real machined object; the accent
    // teal carried per-vertex becomes painted metal rather than a flat stroke.
    let (ip, ic, ii) = instrument_geometry();
    let mut imesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let inormals = vec![[0.0, 1.0, 0.0]; ip.len()];
    let iuvs = vec![[0.0, 0.0]; ip.len()];
    imesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, ip);
    imesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, iuvs);
    imesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, inormals);
    imesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, ic);
    imesh.insert_indices(Indices::U32(ii));

    let imat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        metallic: 0.85,
        perceptual_roughness: 0.35,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(imesh)),
        MeshMaterial3d(imat),
        Transform::default(),
        FluidVisual,
        Visibility::Hidden,
        LAYER,
    ));
}

/// Vertex layout of the polar water disc: a centre vertex followed by
/// `DISC_RINGS` concentric rings of `DISC_SEGMENTS` vertices each.
fn disc_geometry() -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = Vec::with_capacity(1 + DISC_RINGS * DISC_SEGMENTS);
    let mut indices = Vec::with_capacity(DISC_RINGS * DISC_SEGMENTS * 6);

    positions.push([0.0, 0.0, 0.0]);
    for j in 0..DISC_SEGMENTS {
        let th = std::f32::consts::TAU * j as f32 / DISC_SEGMENTS as f32;
        let r = PLATE_R / DISC_RINGS as f32;
        positions.push([r * th.cos(), 0.0, r * th.sin()]);
    }
    for j in 0..DISC_SEGMENTS {
        let a = 1 + j;
        let b = 1 + (j + 1) % DISC_SEGMENTS;
        indices.extend_from_slice(&[0u32, a as u32, b as u32]);
    }
    for i in 1..DISC_RINGS {
        let r_out = PLATE_R * (i + 1) as f32 / DISC_RINGS as f32;
        for j in 0..DISC_SEGMENTS {
            let th = std::f32::consts::TAU * j as f32 / DISC_SEGMENTS as f32;
            positions.push([r_out * th.cos(), 0.0, r_out * th.sin()]);
        }
        let a_base = 1 + (i - 1) * DISC_SEGMENTS;
        let b_base = 1 + i * DISC_SEGMENTS;
        for j in 0..DISC_SEGMENTS {
            let a0 = (a_base + j) as u32;
            let a1 = (a_base + (j + 1) % DISC_SEGMENTS) as u32;
            let b0 = (b_base + j) as u32;
            let b1 = (b_base + (j + 1) % DISC_SEGMENTS) as u32;
            indices.extend_from_slice(&[a0, b0, a1, b1, a1, b0]);
        }
    }
    (positions, indices)
}

/// Builds the graduated bezel, the accent hairline and the tick marks as one
/// flat vertex-coloured mesh lying just above the pool plane.
fn instrument_geometry() -> (Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
    let mut flat = FlatMesh::default();

    let accent = ACCENT.to_srgba();
    let accent = [accent.red, accent.green, accent.blue, accent.alpha];

    flat.annulus(
        BEZEL_INNER,
        BEZEL_OUTER,
        0.020,
        [0.115, 0.125, 0.170, 1.0],
        BEZEL_SEGMENTS,
    );
    flat.annulus(
        HAIRLINE_INNER,
        HAIRLINE_OUTER,
        0.030,
        accent,
        BEZEL_SEGMENTS,
    );

    for k in 0..N_TICKS {
        let cardinal = k % CARDINAL_EVERY == 0;
        let th = std::f32::consts::TAU * k as f32 / N_TICKS as f32;
        let (s, c) = th.sin_cos();
        // Radial unit (c, s) and tangential unit (-s, c) in the (x, z) plane.
        let r0 = if cardinal { 3.00 } else { 3.05 };
        let r1 = if cardinal { 3.46 } else { 3.30 };
        let hw = if cardinal { 0.012 } else { 0.008 };
        let col = if cardinal {
            [0.45, 0.47, 0.56, 1.0]
        } else {
            [0.26, 0.27, 0.34, 1.0]
        };
        let base = flat.positions.len() as u32;
        let corner = |r: f32, w: f32| [c * r - s * w, 0.035, s * r + c * w];
        flat.positions.push(corner(r0, -hw));
        flat.positions.push(corner(r0, hw));
        flat.positions.push(corner(r1, hw));
        flat.positions.push(corner(r1, -hw));
        flat.colors.extend_from_slice(&[col, col, col, col]);
        flat.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    (flat.positions, flat.colors, flat.indices)
}

/// Accumulator for a flat, vertex-coloured mesh (positions, colours, indices).
#[derive(Default)]
struct FlatMesh {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl FlatMesh {
    /// Appends a flat annulus (a ring of quads between two radii) at height `y`.
    fn annulus(&mut self, r_in: f32, r_out: f32, y: f32, color: [f32; 4], segments: usize) {
        let base = self.positions.len() as u32;
        for j in 0..segments {
            let th = std::f32::consts::TAU * j as f32 / segments as f32;
            let (s, c) = th.sin_cos();
            self.positions.push([r_in * c, y, r_in * s]);
            self.positions.push([r_out * c, y, r_out * s]);
            self.colors.push(color);
            self.colors.push(color);
        }
        for j in 0..segments {
            let a0 = base + (2 * j) as u32;
            let b0 = base + (2 * j + 1) as u32;
            let a1 = base + (2 * ((j + 1) % segments)) as u32;
            let b1 = base + (2 * ((j + 1) % segments) + 1) as u32;
            self.indices.extend_from_slice(&[a0, b0, a1, b0, b1, a1]);
        }
    }
}

/// Fits the orthographic scope to the viewport: the instrument's outer radius
/// always occupies [`SCOPE_FRACTION`] of the shorter viewport edge (times the
/// current zoom), so the pool never clips, wide or tall.
pub fn fit_fluid_scope(
    zoom: Res<FluidZoom>,
    mut query: Query<(&Camera, &mut Projection), With<FluidCamera>>,
) {
    for (camera, mut projection) in &mut query {
        let Some(size) = camera.logical_viewport_size() else {
            continue;
        };
        let min_edge = size.x.min(size.y).max(1.0);
        let scale = (2.0 * SCOPE_FIT_R / SCOPE_FRACTION) / min_edge / zoom.0.max(0.01);
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = scale;
        }
    }
}

/// Wheel zoom on the scope, only while the cursor is over the 3D viewport.
/// Rotation and panning stay disabled: a cymascope is a fixed, top-down view.
pub fn zoom_fluid_scope(
    ui_state: Res<UiState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut wheel: MessageReader<MouseWheel>,
    mut zoom: ResMut<FluidZoom>,
) {
    if ui_state.active_module != ActiveModule::Fluid {
        return;
    }
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x <= crate::responsive_panel_width(window.width()) {
        return;
    }
    let scroll = wheel.read().map(|event| event.y).sum::<f32>();
    if scroll == 0.0 {
        return;
    }
    zoom.0 = (zoom.0 * (1.0 + scroll * 0.08)).clamp(0.5, 5.0);
}

/// Quantised identity of the displayed glyph: the analytic mode only needs
/// recomputing when the tone (0.1 Hz step) or the liquid changes.
fn glyph_key(ui: &UiState) -> [u32; 4] {
    [
        (ui.frequency_hz * 10.0).round().max(0.0) as u32,
        ui.fluid_density.round().max(0.0) as u32,
        (ui.fluid_surface_tension * 1.0e6).round().max(0.0) as u32,
        (ui.fluid_depth * 1.0e6).round().max(0.0) as u32,
    ]
}

pub fn update_fluid_mesh(
    ui_state: Res<UiState>,
    query: Query<&Mesh3d, With<FluidHeightmap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: Local<Option<FluidDisplayState>>,
    mut last_log: Local<Option<std::time::Instant>>,
) {
    // Only rebuild the visible module.
    if ui_state.active_module != ActiveModule::Fluid {
        return;
    }

    let Ok(mesh3d) = query.single() else {
        return;
    };
    let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
        return;
    };

    let st = state.get_or_insert_with(|| FluidDisplayState {
        field: vec![0.0; GRID_OUT * GRID_OUT],
        target: vec![0.0; GRID_OUT * GRID_OUT],
        key: [u32::MAX; 4],
    });

    // The placed figure is the analytic Faraday glyph for the current tone,
    // regenerated only when the tone or the liquid changes. The physics
    // (Bessel modes + capillary–gravity dispersion) lives in this viewer's
    // `render::glyph` module; the crate's fluid solver is a separate,
    // general-purpose simulation.
    let key = glyph_key(&ui_state);
    if st.key != key {
        let liquid = cymatrox::fluid::LiquidSpec {
            density: ui_state.fluid_density,
            surface_tension: ui_state.fluid_surface_tension,
            depth: ui_state.fluid_depth,
            damping: ui_state.fluid_damping,
            gravity: 9.81,
        };
        glyph::rasterise_glyph(
            ui_state.frequency_hz,
            PLATE_RADIUS_M,
            &liquid,
            &mut st.target,
        );
        st.key = key;
    }

    {
        let now = std::time::Instant::now();
        let do_log = last_log
            .as_ref()
            .map(|t| now.duration_since(*t) >= std::time::Duration::from_secs(1))
            .unwrap_or(true);
        if do_log {
            let max_h = st.target.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
            debug!(
                "[fluid3d] update: {} nodes, max |h|={:.3e} m",
                st.target.len(),
                max_h
            );
            *last_log = Some(now);
        }
    }

    // Time-smooth the display field toward the glyph, so a tone change
    // cross-fades the previous figure into the new one.
    for (smoothed, goal) in st.field.iter_mut().zip(st.target.iter()) {
        *smoothed += 0.8 * (*goal - *smoothed);
    }
    let hlin: Vec<f32> = st.field.clone();

    // Spatial smoothing: a light 3x3 convolution takes the hard sampling edge
    // off the high-order modes without erasing the fine nodal network.
    let hlin = smooth_field(&hlin, GRID_OUT);

    // Normalise the displayed relief: the glyph is a physical height field in
    // metres (tens of micrometres), far too small to shade at the dish scale,
    // so scale it per frame until its peak reaches `DISPLAY_TARGET_HEIGHT`.
    let max_abs = hlin.iter().fold(0.0f32, |a, &b| a.max(b.abs())).max(1e-12);
    let hscale = DISPLAY_TARGET_HEIGHT / max_abs;

    // Autoscale the froth response to this frame's live slope so a developing
    // glyph reads immediately and a calm pool stays clean.
    let dx_world = 2.0 * PLATE_R / (GRID_OUT as f32 - 1.0);
    let slope_ref = (max_slope(&hlin, GRID_OUT) * hscale / dx_world).max(MIN_SLOPE_REF);

    // Rebuild the polar disc, sampling the field at each vertex and shading the
    // relief via surface normals.
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(1 + DISC_RINGS * DISC_SEGMENTS);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(1 + DISC_RINGS * DISC_SEGMENTS);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(1 + DISC_RINGS * DISC_SEGMENTS);

    let mut push = |x: f32, z: f32| {
        let (p, n, c) = water_vertex(x, z, &hlin, slope_ref, hscale);
        positions.push(p);
        normals.push(n);
        colors.push(c);
    };
    push(0.0, 0.0);
    for i in 1..=DISC_RINGS {
        let r = PLATE_R * i as f32 / DISC_RINGS as f32;
        for j in 0..DISC_SEGMENTS {
            let th = std::f32::consts::TAU * j as f32 / DISC_SEGMENTS as f32;
            push(r * th.cos(), r * th.sin());
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
}

/// Samples the display field at a plate position and returns the displaced
/// position, the surface normal (from field gradients) and the surface albedo.
///
/// With the surface now physically lit, this no longer bakes the ring-light
/// pattern: the brightness comes from the raking key and the reflected
/// environment acting on the normal. What the albedo carries is the *material*
/// of the water — a deep, dark blue body, a little white froth on the steepest
/// crests, and a brighter meniscus where the film climbs the dish wall.
fn water_vertex(
    x: f32,
    z: f32,
    field: &[f32],
    slope_ref: f32,
    hscale: f32,
) -> ([f32; 3], [f32; 3], [f32; 4]) {
    let h = displayed_height_at(x, z, field, hscale);
    let eps = PLATE_R / DISC_RINGS as f32;
    let dhx = (displayed_height_at(x + eps, z, field, hscale)
        - displayed_height_at(x - eps, z, field, hscale))
        / (2.0 * eps);
    let dhz = (displayed_height_at(x, z + eps, field, hscale)
        - displayed_height_at(x, z - eps, field, hscale))
        / (2.0 * eps);
    let n = Vec3::new(-dhx, 1.0, -dhz).normalize();

    // Froth: the steepest crests whiten slightly (real standing waves tear a
    // little), normalised by the live slope reference so it tracks amplitude.
    let slope = (dhx * dhx + dhz * dhz).sqrt();
    let s = (slope / slope_ref).clamp(0.0, 1.0);
    let foam = smoothstep(0.72, 1.0, s);
    // The meniscus where the water climbs the wall catches the light.
    let glass = smoothstep(0.90, 1.0, (x * x + z * z).sqrt() / PLATE_R);

    // Deep water body, faintly cool, with froth and meniscus brightening it.
    let deep = Vec3::new(0.035, 0.075, 0.11);
    let col = deep + Vec3::new(0.55, 0.62, 0.70) * foam + Vec3::splat(0.20) * glass;

    ([x, h, z], [n.x, n.y, n.z], [col.x, col.y, col.z, 1.0])
}

/// Largest finite-difference gradient magnitude in a row-major square field
/// (per grid cell; the caller converts it to world slope).
fn max_slope(field: &[f32], w: usize) -> f32 {
    let mut best = 0.0f32;
    for y in 0..w {
        for x in 0..w {
            let at = |x: usize, y: usize| field[y * w + x];
            let c = at(x, y);
            let l = if x > 0 { at(x - 1, y) } else { c };
            let r = if x + 1 < w { at(x + 1, y) } else { c };
            let u = if y > 0 { at(x, y - 1) } else { c };
            let d = if y + 1 < w { at(x, y + 1) } else { c };
            let gx = (r - l) * 0.5;
            let gz = (d - u) * 0.5;
            best = best.max((gx * gx + gz * gz).sqrt());
        }
    }
    best
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Displaced surface height at a plate position: the normalised field scaled to
/// display units, with a gentle rim falloff so the surface meets the bezel.
fn displayed_height_at(x: f32, z: f32, field: &[f32], hscale: f32) -> f32 {
    let r = (x * x + z * z).sqrt();
    if r > PLATE_R {
        return 0.0;
    }
    let raw = sample_bilinear(
        (x / PLATE_R).clamp(-1.0, 1.0),
        (z / PLATE_R).clamp(-1.0, 1.0),
        field,
    );
    if !raw.is_finite() {
        return 0.0;
    }
    let h = (raw * hscale).clamp(-MAX_DISPLAY_HEIGHT, MAX_DISPLAY_HEIGHT);
    let rim = 1.0 - (r / PLATE_R).max(0.0).powi(2);
    h * (0.35 + 0.65 * rim)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_is_a_center_plus_every_ring() {
        let (positions, indices) = disc_geometry();
        assert_eq!(positions.len(), 1 + DISC_RINGS * DISC_SEGMENTS);
        assert_eq!(indices.len() % 3, 0);
        // Center fan + two triangles per quad in each ring gap.
        assert_eq!(indices.len() / 3, DISC_SEGMENTS * (2 * DISC_RINGS - 1));
        assert!(indices.iter().all(|&i| (i as usize) < positions.len()));
    }

    #[test]
    fn instrument_geometry_is_well_formed() {
        let (positions, colors, indices) = instrument_geometry();
        assert_eq!(positions.len(), colors.len());
        assert!(!indices.is_empty());
        assert_eq!(indices.len() % 3, 0);
        assert!(indices.iter().all(|&i| (i as usize) < positions.len()));
    }

    #[test]
    fn scope_fit_keeps_the_pool_inside_the_shorter_edge() {
        let min_edge = 700.0f32;
        let scale = (2.0 * SCOPE_FIT_R / SCOPE_FRACTION) / min_edge;
        let visible_min_world = min_edge * scale;
        assert!((visible_min_world - 2.0 * SCOPE_FIT_R / SCOPE_FRACTION).abs() < 1e-3);
        // The pool diameter is a strict subset of the fitted scope.
        let pool_fraction = (2.0 * PLATE_R) / visible_min_world;
        assert!(pool_fraction > 0.0 && pool_fraction < SCOPE_FRACTION);
    }

    #[test]
    fn displayed_height_is_bounded_and_flat_outside_the_pool() {
        let field = vec![0.5f32; GRID_OUT * GRID_OUT];
        for &(x, z) in &[
            (0.0, 0.0),
            (PLATE_R, 0.0),
            (PLATE_R * 0.5, PLATE_R * 0.5),
            (PLATE_R * 2.0, 0.0),
        ] {
            let h = displayed_height_at(x, z, &field, 1.0);
            assert!(h.is_finite());
            assert!(h.abs() <= MAX_DISPLAY_HEIGHT);
        }
        assert_eq!(displayed_height_at(PLATE_R * 1.5, 0.0, &field, 1.0), 0.0);
    }
}
