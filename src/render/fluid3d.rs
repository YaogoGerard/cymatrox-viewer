use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::ScalingMode;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::sync::OnceLock;

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

/// How far the instrument is lifted above the viewport centre, as a fraction of
/// its own on-screen radius, so the dish and bezel clear the frequency ruler
/// that the HUD draws along the bottom edge of the 3D view. A world-space offset
/// of `SCOPE_LIFT_FRACTION * SCOPE_FIT_R` maps to exactly this many pixels
/// whatever the window size (the fit scale is proportional to the shorter edge).
pub const SCOPE_LIFT_FRACTION: f32 = 0.18;

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

/// Target maximum *slope* of the displayed relief (dimensionless, dy/dx in
/// world units). The glyph is normalised per frame so its steepest slope always
/// reaches this value. Normalising the slope rather than the peak height is what
/// keeps the pool reading as water at every tone: a low tone throws a few big
/// waves, a high tone a fine shimmer, instead of the high-order modes (two
/// dozen nodal rings at 440 Hz) turning into a spiky fence. The physical
/// Faraday amplitude is itself only a small fraction of the wavelength, so a
/// bounded aspect ratio is also the physical choice.
const TARGET_SLOPE: f32 = 1.05;
/// Floor of the live slope reference (display units) used to autoscale the
/// froth response. A calm pool stays dark instead of amplifying noise.
const MIN_SLOPE_REF: f32 = 0.05;

/// Fraction of the base figure's steepest slope carried by the continuous
/// surface motion (travelling ripples + slosh). Large enough that the ring-light
/// glints visibly travel across the pool, small enough that the Faraday figure
/// stays clearly dominant.
const MOTION_SLOPE_FRACTION: f32 = 0.33;
/// Samples in the radial ripple lookup table (the ripple is a function of `r`).
const RIP_TABLE_N: usize = 2048;
/// Radial wavenumbers (rings across the pool), temporal frequencies (Hz) and
/// relative weights of the outward-travelling ripples that keep the surface
/// alive. Irrational-ish ratios avoid an obvious repeating beat.
const RIP_WAVES: [f32; 3] = [4.0, 7.0, 11.0];
const RIP_FREQS: [f32; 3] = [1.05, 1.55, 2.15];
const RIP_WEIGHTS: [f32; 3] = [0.62, 0.28, 0.14];
/// Slow slosh: a gentle tilt that precesses at this rate (Hz).
const SLOSH_HZ: f32 = 0.22;
const SLOSH_WEIGHT: f32 = 0.45;
/// Settling spring, deliberately under-damped so a tone change rises, overshoots
/// a hair and settles instead of swapping to a pre-drawn figure instantly.
const FLUID_SPRING_K: f32 = 16.0;
const FLUID_SPRING_DAMPING: f32 = 0.62;
/// Prevent a numerically unstable frame from turning the water into a giant
/// solid volume (~4 mm of water at the display scale). The physical values
/// remain unchanged in the simulation.
const MAX_DISPLAY_HEIGHT: f32 = 0.5;
/// Layout grid resolution used to sample the Faraday field. The analytic glyph
/// carries several tens of nodal rings at the top of the audio range (about
/// forty at 1 kHz), so this must resolve well under a ring spacing: at 512 the
/// finest mode is sampled with ~5 cells per nodal ring, which the bilinear
/// lookup then smooths.
const GRID_OUT: usize = 512;

/// Water disc tessellation: concentric rings of vertices, uniform angular
/// resolution. A polar mesh gives a clean circular pool edge (a square grid
/// would leave flat corners sticking out around the dish). The ring count has
/// to resolve the finest radial mode (~40 nodal rings at 1 kHz), hence the
/// generous figure; the segment count only has to resolve the azimuthal petals.
const DISC_RINGS: usize = 220;
const DISC_SEGMENTS: usize = 256;

/// Euler angles of the dedicated key that lights the brushed-metal instrument on
/// the liquid layer. The water itself is shaded by the light ring below.
const KEY_EULER: (f32, f32, f32) = (-1.02, 0.55, 0.0);

/// Geometry of the coaxial light-ring illuminator the pool is photographed with
/// (the classic CymaScope arrangement): a ring of light above the water, viewed
/// straight down. A patch of surface glints when its tilt reflects the view ray
/// into the ring, so the bright lines trace contours of the wave *slope* — which
/// is exactly what a CymaGlyph is. `RING_ELEV` is the ring's elevation above the
/// horizon and `RING_SIGMA` the angular width of the highlight lobe: bigger is
/// a softer, wider glow (a fatter ring / rougher water).
const RING_ELEV: f32 = 0.82;
const RING_SIGMA: f32 = 0.17;
/// Slope range and resolution of the glint lookup table. The highlight lobe is
/// centred near slope 0.4 and is black well before 4, so the table is exact
/// enough there; beyond it the highlight has faded out.
const GLINT_SLOPE_MAX: f32 = 4.0;
const GLINT_TABLE_N: usize = 1024;

/// Zoom on the scope (mouse wheel). `1.0` is the fitted default framing; larger
/// values move the camera closer. Lives in a resource because both the fit and
/// the zoom systems read it.
#[derive(Resource)]
pub struct FluidZoom(pub f32);

/// One water vertex ready to be shaded: where it is, its precomputed rim
/// highlight (`glass`) and its interpolated + animated display height and slope.
struct WaterSample {
    x: f32,
    z: f32,
    glass: f32,
    h: f32,
    dhx: f32,
    dhz: f32,
}

/// Cached per-vertex geometry for one settled glyph: the display height and the
/// two slope components at every disc vertex, in display units. Producing these
/// needs the expensive rasterise + smooth + bilinear sampling, so they are
/// computed once per tone and only interpolated (never re-sampled) thereafter.
#[derive(Default)]
struct GlyphCache {
    h: Vec<f32>,
    dhx: Vec<f32>,
    dhz: Vec<f32>,
    hscale: f32,
    slope_ref: f32,
    /// `hscale · max|field|`: the crest normalisation.
    norm: f32,
    valid: bool,
}

impl GlyphCache {
    fn blank(n: usize) -> Self {
        Self {
            h: vec![0.0; n],
            dhx: vec![0.0; n],
            dhz: vec![0.0; n],
            hscale: 1.0,
            slope_ref: MIN_SLOPE_REF,
            norm: 1.0,
            valid: false,
        }
    }

    /// Sample the freshly rasterised (and smoothed) glyph onto the disc vertices.
    fn from_grid(grid: &[f32], xs: &[f32], zs: &[f32]) -> Self {
        let smoothed = smooth_field(grid, GRID_OUT);
        let max_abs = smoothed
            .iter()
            .fold(0.0f32, |a, &b| a.max(b.abs()))
            .max(1e-12);
        let dx_world = 2.0 * PLATE_R / (GRID_OUT as f32 - 1.0);
        let gmax = max_slope(&smoothed, GRID_OUT).max(1e-12);
        let hscale = (TARGET_SLOPE * dx_world / gmax).min(MAX_DISPLAY_HEIGHT / max_abs);
        let slope_ref = (gmax * hscale / dx_world).max(MIN_SLOPE_REF);
        let eps = PLATE_R / DISC_RINGS as f32;

        let mut h = Vec::with_capacity(xs.len());
        let mut dhx = Vec::with_capacity(xs.len());
        let mut dhz = Vec::with_capacity(xs.len());
        for (&x, &z) in xs.iter().zip(zs.iter()) {
            h.push(displayed_height_at(x, z, &smoothed, hscale));
            dhx.push(
                (displayed_height_at(x + eps, z, &smoothed, hscale)
                    - displayed_height_at(x - eps, z, &smoothed, hscale))
                    / (2.0 * eps),
            );
            dhz.push(
                (displayed_height_at(x, z + eps, &smoothed, hscale)
                    - displayed_height_at(x, z - eps, &smoothed, hscale))
                    / (2.0 * eps),
            );
        }
        Self {
            h,
            dhx,
            dhz,
            hscale,
            slope_ref,
            norm: hscale * max_abs,
            valid: true,
        }
    }

    /// The interpolated cache `a + t·(b − a)`, i.e. the figure actually on the
    /// water while a tone change is settling.
    fn blend(a: &Self, b: &Self, t: f32) -> Self {
        let mut h = Vec::with_capacity(a.h.len());
        let mut dhx = Vec::with_capacity(a.h.len());
        let mut dhz = Vec::with_capacity(a.h.len());
        for i in 0..a.h.len() {
            h.push(lerp(a.h[i], b.h[i], t));
            dhx.push(lerp(a.dhx[i], b.dhx[i], t));
            dhz.push(lerp(a.dhz[i], b.dhz[i], t));
        }
        Self {
            h,
            dhx,
            dhz,
            hscale: lerp(a.hscale, b.hscale, t),
            slope_ref: lerp(a.slope_ref, b.slope_ref, t),
            norm: lerp(a.norm, b.norm, t),
            valid: a.valid || b.valid,
        }
    }
}

/// Everything the per-frame water update carries between frames. The costly
/// per-tone work — rasterise a Faraday mode, smooth it, sample it onto the disc —
/// happens only in [`GlyphCache::from_grid`]; each frame merely interpolates the
/// two cached glyphs and adds a cheap analytic surface motion. That is what lets
/// the pool move continuously at full frame rate instead of freezing on a
/// pre-drawn figure.
pub struct FluidDisplayState {
    cur: GlyphCache,
    prev: GlyphCache,
    key: [u32; 4],
    /// Hand-off from `prev` to `cur`: 0 at the old figure, 1 at the new, with a
    /// small overshoot from the under-damped spring.
    progress: f32,
    velocity: f32,
    /// Wall-clock time of the previous update (seconds), for the spring `dt`.
    last_time: f32,
    /// Disc vertex positions in mesh order. The vertices never move and the
    /// angles are too costly to recompute every frame.
    xs: Vec<f32>,
    zs: Vec<f32>,
    /// Unit vectors of the `DISC_SEGMENTS` angular steps, shared by every ring so
    /// no trigonometry runs in the per-vertex loop.
    cosj: Vec<f32>,
    sinj: Vec<f32>,
    /// Scratch grid the analytic glyph is rasterised into before smoothing.
    grid: Vec<f32>,
    /// Largest world-space gradient of the *unit* surface motion; used to scale
    /// the motion to a fixed fraction of the figure's slope.
    motion_grad: f32,
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
        // A generated studio environment (no asset needed): the brushed-metal
        // instrument on this camera needs something to reflect. Bright at the
        // zenith, falling to a dark nadir, and kept restrained so it never
        // competes with the baked cymascope figure.
        EnvironmentMapLight::hemispherical_gradient(
            &mut images,
            Color::srgb(0.34, 0.41, 0.53),
            Color::srgb(0.06, 0.08, 0.12),
            Color::srgb(0.008, 0.011, 0.016),
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
        // Straight down; up = -Z keeps +X to the right on screen. The camera is
        // nudged along +Z (screen up) so the instrument rides above the bottom
        // HUD instead of sitting dead-centre in the viewport.
        Transform::from_xyz(0.0, 30.0, SCOPE_LIFT_FRACTION * SCOPE_FIT_R).looking_at(
            Vec3::new(0.0, 0.0, SCOPE_LIFT_FRACTION * SCOPE_FIT_R),
            Vec3::NEG_Z,
        ),
        FluidCamera,
        crate::render::camera::ModuleViewportCamera,
        Visibility::Hidden,
        LAYER,
    ));

    // A dedicated key on the liquid layer. The water itself is unlit — its
    // figure is baked (`water_vertex`) — but this light shades the brushed-metal
    // instrument, and its direction is the one the caustic baking treats as the
    // light coming into the pool.
    commands.spawn((
        DirectionalLight {
            illuminance: 14_000.0,
            color: Color::srgb(0.94, 0.97, 1.0),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            KEY_EULER.0,
            KEY_EULER.1,
            KEY_EULER.2,
        )),
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

    // Water: the figure is baked per-vertex (see `water_vertex`) and drawn
    // unlit, so the material keeps its base colour white and simply shows the
    // authored crest/caustic shading. `perceptual_roughness`/`reflectance` are
    // left at sensible values for the (unused) lit path.
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        metallic: 0.0,
        perceptual_roughness: 0.28,
        reflectance: 0.35,
        unlit: true,
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
    time: Res<Time>,
    query: Query<&Mesh3d, With<FluidHeightmap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: Local<Option<FluidDisplayState>>,
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

    let n_vert = 1 + DISC_RINGS * DISC_SEGMENTS;
    let st = state.get_or_insert_with(|| {
        let mut cosj = Vec::with_capacity(DISC_SEGMENTS);
        let mut sinj = Vec::with_capacity(DISC_SEGMENTS);
        for j in 0..DISC_SEGMENTS {
            let th = std::f32::consts::TAU * j as f32 / DISC_SEGMENTS as f32;
            cosj.push(th.cos());
            sinj.push(th.sin());
        }
        let mut xs = Vec::with_capacity(n_vert);
        let mut zs = Vec::with_capacity(n_vert);
        xs.push(0.0);
        zs.push(0.0);
        for i in 1..=DISC_RINGS {
            let r = PLATE_R * i as f32 / DISC_RINGS as f32;
            for j in 0..DISC_SEGMENTS {
                xs.push(r * cosj[j]);
                zs.push(r * sinj[j]);
            }
        }
        FluidDisplayState {
            cur: GlyphCache::blank(n_vert),
            prev: GlyphCache::blank(n_vert),
            key: [u32::MAX; 4],
            progress: 0.0,
            velocity: 0.0,
            last_time: 0.0,
            xs,
            zs,
            cosj,
            sinj,
            grid: vec![0.0; GRID_OUT * GRID_OUT],
            motion_grad: estimate_motion_grad(),
        }
    });

    // Frame time for the settling spring, capped so a stalled frame cannot blow
    // up the integration.
    let now = time.elapsed_secs();
    let dt = if st.last_time == 0.0 {
        1.0 / 60.0
    } else {
        (now - st.last_time).clamp(0.0, 0.05)
    };
    st.last_time = now;

    // The placed figure is the analytic Faraday glyph for the current tone,
    // regenerated only when the tone or the liquid changes. The physics
    // (Bessel modes + capillary–gravity dispersion) lives in this viewer's
    // `render::glyph` module; the crate's fluid solver is a separate,
    // general-purpose simulation.
    if st.key != glyph_key(&ui_state) {
        st.key = glyph_key(&ui_state);

        // The figure currently on the water becomes the outgoing one, so the
        // spring below morphs it into the new mode instead of cutting to it.
        if st.cur.valid {
            st.prev = GlyphCache::blend(&st.prev, &st.cur, st.progress);
            st.prev.valid = true;
        }

        let liquid = cymatrox::fluid::LiquidSpec {
            density: ui_state.fluid_density,
            surface_tension: ui_state.fluid_surface_tension,
            depth: ui_state.fluid_depth,
            damping: ui_state.fluid_damping,
            gravity: 9.81,
        };
        glyph::rasterise_glyph(ui_state.frequency_hz, PLATE_RADIUS_M, &liquid, &mut st.grid);
        st.cur = GlyphCache::from_grid(&st.grid, &st.xs, &st.zs);

        // First figure ever: keep the normalisation identical so only the relief
        // grows out of the calm pool instead of the whole tone jumping.
        if !st.prev.valid {
            st.prev.hscale = st.cur.hscale;
            st.prev.slope_ref = st.cur.slope_ref;
            st.prev.norm = st.cur.norm;
            st.prev.valid = true;
        }

        st.progress = 0.0;
        st.velocity = 0.0;
    }

    // Under-damped spring on the hand-off progress: the new figure rises,
    // overshoots a hair and settles — the water *responds* to the tone rather
    // than presenting a pre-drawn figure.
    let stiffness = FLUID_SPRING_K;
    let damping = 2.0 * FLUID_SPRING_DAMPING * stiffness.sqrt();
    let accel = stiffness * (1.0 - st.progress) - damping * st.velocity;
    st.velocity += accel * dt;
    st.progress += st.velocity * dt;
    let p = st.progress;

    let slope_ref = lerp(st.prev.slope_ref, st.cur.slope_ref, p);
    let norm = lerp(st.prev.norm, st.cur.norm, p).max(1e-6);
    let motion_scale = MOTION_SLOPE_FRACTION * slope_ref / st.motion_grad.max(1e-6);

    // Rebuild the disc: interpolate the two cached glyphs, add the live surface
    // motion and shade. No bilinear sampling happens here, so this stays cheap
    // enough to run every frame. The radial part of the motion is constant around
    // each ring, so it is evaluated once per ring and reused for every segment.
    let motion = MotionParams::new(now, motion_scale);
    let inv_plate = 1.0 / PLATE_R;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n_vert);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n_vert);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n_vert);

    // Hub vertex (r = 0).
    {
        let (mh, mdx, mdz) = motion_at(0.0, 0.0, &motion);
        let h = lerp(st.prev.h[0], st.cur.h[0], p) + mh;
        let dhx = lerp(st.prev.dhx[0], st.cur.dhx[0], p) + mdx;
        let dhz = lerp(st.prev.dhz[0], st.cur.dhz[0], p) + mdz;
        let (pos, nrm, col) = shade_surface(
            WaterSample {
                x: 0.0,
                z: 0.0,
                glass: 0.0,
                h,
                dhx,
                dhz,
            },
            slope_ref,
            norm,
        );
        positions.push(pos);
        normals.push(nrm);
        colors.push(col);
    }
    let mut k = 1;
    for i in 1..=DISC_RINGS {
        let rn = i as f32 / DISC_RINGS as f32;
        let r2 = rn * rn;
        let env = 1.0 - r2;
        let (ripple, d_ripple) = ripple_radial(rn, &motion);
        // Radial derivative of the motion field with the slosh split out, so the
        // per-segment cost is a handful of multiplies.
        let a_ring = env * d_ripple - 2.0 * rn * ripple;
        let b_ring = -2.0 * rn;
        let du_coef = env * motion.slosh_cos * SLOSH_WEIGHT;
        let dv_coef = env * motion.slosh_sin * SLOSH_WEIGHT;
        let glass = smoothstep(0.90, 1.0, rn);

        for j in 0..DISC_SEGMENTS {
            let cj = st.cosj[j];
            let sj = st.sinj[j];
            let u = rn * cj;
            let v = rn * sj;
            let slosh = (u * motion.slosh_cos + v * motion.slosh_sin) * SLOSH_WEIGHT;
            let surface = ripple + slosh;
            let d_dr = a_ring + b_ring * slosh;

            let mh = motion.scale * env * surface;
            let mdx = motion.scale * inv_plate * (d_dr * cj + du_coef);
            let mdz = motion.scale * inv_plate * (d_dr * sj + dv_coef);

            let h = lerp(st.prev.h[k], st.cur.h[k], p) + mh;
            let dhx = lerp(st.prev.dhx[k], st.cur.dhx[k], p) + mdx;
            let dhz = lerp(st.prev.dhz[k], st.cur.dhz[k], p) + mdz;
            let (pos, nrm, col) = shade_surface(
                WaterSample {
                    x: u * PLATE_R,
                    z: v * PLATE_R,
                    glass,
                    h,
                    dhx,
                    dhz,
                },
                slope_ref,
                norm,
            );
            positions.push(pos);
            normals.push(nrm);
            colors.push(col);
            k += 1;
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
}

/// Radial ripple lookup: `[sin, cos][component][i]` for `r_i = i / (N − 1)`.
///
/// The travelling ripples are functions of radius, so tabulating their spatial
/// part turns the per-cell cost from three `sin` calls into three interpolated
/// lookups; only the small temporal part is recomputed each frame.
fn ripple_table() -> &'static [[[f32; RIP_TABLE_N]; 3]; 2] {
    static TABLE: OnceLock<[[[f32; RIP_TABLE_N]; 3]; 2]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut sin_t = [[0.0f32; RIP_TABLE_N]; 3];
        let mut cos_t = [[0.0f32; RIP_TABLE_N]; 3];
        for comp in 0..3 {
            for (i, (s, c)) in sin_t[comp]
                .iter_mut()
                .zip(cos_t[comp].iter_mut())
                .enumerate()
            {
                let r = i as f32 / (RIP_TABLE_N - 1) as f32;
                let a = std::f32::consts::TAU * RIP_WAVES[comp] * r;
                *s = a.sin();
                *c = a.cos();
            }
        }
        [sin_t, cos_t]
    })
}

/// Precomputed temporal factors of the surface motion for one frame. The
/// travelling ripples and the slosh depend on time only through these, so they
/// are evaluated once per frame rather than once per vertex.
struct MotionParams {
    /// `(sin, cos)` of `TAU · frequency · t` for each ripple component.
    temporal: [(f32, f32); 3],
    slosh_sin: f32,
    slosh_cos: f32,
    /// Display-unit amplitude that scales the unit motion to the wanted slope.
    scale: f32,
}

impl MotionParams {
    fn new(t: f32, scale: f32) -> Self {
        let temporal =
            std::array::from_fn(|c| (std::f32::consts::TAU * RIP_FREQS[c] * t).sin_cos());
        let (slosh_sin, slosh_cos) = (std::f32::consts::TAU * SLOSH_HZ * t).sin_cos();
        Self {
            temporal,
            slosh_sin,
            slosh_cos,
            scale,
        }
    }
}

/// Glint strength vs. surface slope, tabulated once. Evaluating the coaxial
/// highlight directly needs `acos` + `exp` per vertex; the curve is smooth and
/// monotone in slope, so a small lerped table is indistinguishable.
fn glint_table() -> &'static [f32; GLINT_TABLE_N] {
    static TABLE: OnceLock<[f32; GLINT_TABLE_N]> = OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let slope = i as f32 / (GLINT_TABLE_N - 1) as f32 * GLINT_SLOPE_MAX;
            // Tilt of the surface normal from vertical is `atan(slope)`.
            let reflected_elev = std::f32::consts::FRAC_PI_2 - 2.0 * slope.atan();
            let d = reflected_elev - RING_ELEV;
            (-(d * d) / (RING_SIGMA * RING_SIGMA)).exp()
        })
    })
}

fn glint_at(slope: f32) -> f32 {
    let t = (slope / GLINT_SLOPE_MAX).clamp(0.0, 1.0) * (GLINT_TABLE_N - 1) as f32;
    let i0 = t as usize;
    let i1 = (i0 + 1).min(GLINT_TABLE_N - 1);
    let f = t - i0 as f32;
    let tab = glint_table();
    tab[i0] * (1.0 - f) + tab[i1] * f
}

/// Radial profile of the travelling ripples at normalised radius `r`, as the
/// value and its radial derivative. Depends only on `r`, so the per-vertex loop
/// evaluates it once per mesh ring rather than once per vertex.
fn ripple_radial(r: f32, m: &MotionParams) -> (f32, f32) {
    use std::f32::consts::TAU;
    let table = ripple_table();
    let (sin_t, cos_t) = (&table[0], &table[1]);
    let rt = r * (RIP_TABLE_N - 1) as f32;
    let i0 = rt as usize;
    let i1 = (i0 + 1).min(RIP_TABLE_N - 1);
    let f = rt - i0 as f32;

    let mut ripple = 0.0f32;
    let mut d_ripple = 0.0f32;
    for comp in 0..3 {
        let sa = sin_t[comp][i0] * (1.0 - f) + sin_t[comp][i1] * f;
        let ca = cos_t[comp][i0] * (1.0 - f) + cos_t[comp][i1] * f;
        let (ts, tc) = m.temporal[comp];
        // sin(TAU·n·r − TAU·f·t) = sin(a)cos(t) − cos(a)sin(t).
        ripple += RIP_WEIGHTS[comp] * (sa * tc - ca * ts);
        // d/dr of the same term: TAU·n·[cos(a)cos(t) + sin(a)sin(t)].
        d_ripple += RIP_WEIGHTS[comp] * TAU * RIP_WAVES[comp] * (ca * tc + sa * ts);
    }
    (ripple, d_ripple)
}

/// Surface motion at a plate position: a few outward-travelling capillary ripples
/// plus a slow slosh, returned as the height and its two world-space gradients,
/// already scaled to display units. Zero outside the disc and faded to zero at
/// the contact line.
///
/// This is a pure *shape* modulation. A global vertical oscillation — the literal
/// Faraday standing wave — would be rescaled away by the per-frame slope
/// normalisation (scaling every height scales every slope equally), which is why
/// a naively "waved" pool still reads as frozen. Moving the *nodal network* is
/// what actually animates the picture.
fn motion_at(x: f32, z: f32, m: &MotionParams) -> (f32, f32, f32) {
    let u = x / PLATE_R;
    let v = z / PLATE_R;
    let r2 = u * u + v * v;
    if r2 >= 1.0 || m.scale == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let r = r2.sqrt();
    let env = 1.0 - r2;

    let (ripple, d_ripple) = ripple_radial(r, m);

    let slosh = (u * m.slosh_cos + v * m.slosh_sin) * SLOSH_WEIGHT;
    let d_slosh_du = m.slosh_cos * SLOSH_WEIGHT;
    let d_slosh_dv = m.slosh_sin * SLOSH_WEIGHT;

    // `env` multiplies `surface`; differentiate radially, then split into u/v.
    let surface = ripple + slosh;
    let d_dr = -2.0 * r * surface + env * d_ripple;
    let (u_r, v_r) = if r > 1e-6 { (u / r, v / r) } else { (0.0, 0.0) };
    let dm_du = d_dr * u_r + env * d_slosh_du;
    let dm_dv = d_dr * v_r + env * d_slosh_dv;

    let inv_r = 1.0 / PLATE_R;
    (
        m.scale * env * surface,
        m.scale * inv_r * dm_du,
        m.scale * inv_r * dm_dv,
    )
}

/// Largest world-space gradient of the *unit* surface motion, sampled over a few
/// times and a dense disc. Used once to scale the motion to a fixed fraction of
/// the figure's slope at any tone.
fn estimate_motion_grad() -> f32 {
    let mut best = 1e-6f32;
    for ti in 0..8 {
        let m = MotionParams::new(ti as f32 * 0.37, 1.0);
        for gy in 0..128 {
            let z = (gy as f32 / 127.0 * 2.0 - 1.0) * PLATE_R;
            for gx in 0..128 {
                let x = (gx as f32 / 127.0 * 2.0 - 1.0) * PLATE_R;
                let (_, dx, dz) = motion_at(x, z, &m);
                best = best.max((dx * dx + dz * dz).sqrt());
            }
        }
    }
    best
}

/// Shades one water vertex from its (already interpolated + animated) display
/// height and slope, returning the displaced position, the surface normal and
/// the colour. `glass` is the precomputed rim highlight for the vertex's ring.
///
/// The water is drawn **unlit**, so the whole CymaGlyph is baked here, following
/// how the instrument actually captures one: an ultra-pure water surface viewed
/// from straight above and lit by a coaxial **ring light**, so the picture is
/// essentially the *reflection* of that ring. The reflection fires wherever the
/// surface tilts strongly enough to bend the view ray out to the ring — a
/// contour of the wave slope — which is what draws the bright nodal lines and
/// petals over an otherwise dark pool. Froth whitens the steepest crests and a
/// brighter meniscus marks the film climbing the dish wall.
fn shade_surface(v: WaterSample, slope_ref: f32, norm: f32) -> ([f32; 3], [f32; 3], [f32; 4]) {
    let WaterSample {
        x,
        z,
        glass,
        h,
        dhx,
        dhz,
    } = v;
    let n = Vec3::new(-dhx, 1.0, -dhz).normalize();

    // Froth: the steepest crests whiten slightly (real standing waves tear a
    // little), normalised by the live slope reference so it tracks amplitude.
    let slope = (dhx * dhx + dhz * dhz).sqrt();
    let s = (slope / slope_ref).clamp(0.0, 1.0);
    let foam = smoothstep(0.72, 1.0, s);

    // Coaxial light-ring reflection. A flat patch sends the view ray straight
    // back up (reflected elevation 90°); tilting the surface by θ lowers it to
    // 90° − 2θ. So the ring at `RING_ELEV` is caught where the wave slope
    // reaches (90° − RING_ELEV)/2, and the highlight follows that slope contour.
    // Tabulated over the slope so the hot loop avoids `acos` + `exp` per vertex.
    let glint = glint_at(slope);

    // The illuminated water: a dark body that still carries a faint image of the
    // wave (so the whole figure reads, not only where it catches the ring), with
    // the ring reflection added on top as the bright, near-white cymascope lines.
    let crest = (h / norm.max(1e-12)).clamp(-1.0, 1.0) * 0.5 + 0.5;
    let deep = Vec3::new(0.10, 0.17, 0.24) * (0.18 + 0.82 * crest);
    let col = deep
        + Vec3::new(0.95, 1.00, 1.08) * glint
        + Vec3::new(0.45, 0.52, 0.60) * foam
        + Vec3::new(0.18, 0.22, 0.26) * glass;

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

/// Linear interpolation; `t` may exceed `[0, 1]` (the settling spring overshoots
/// slightly), which deliberately extrapolates.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
