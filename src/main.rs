mod ui;
use bevy::camera::{visibility::RenderLayers, CameraOutputMode};
use bevy::log::{debug, error, info};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy_egui::{
    EguiGlobalSettings, EguiPlugin, EguiPreUpdateSet, EguiPrimaryContextPass, PrimaryEguiContext,
};
use bevy_panorbit_camera::PanOrbitCameraPlugin;

mod audio;
mod render;
mod sim;

#[derive(Resource)]
pub struct VoiceReceiver {
    pub rx: crossbeam_channel::Receiver<audio::VoiceData>,
}

#[derive(Resource)]
pub struct PrevModule(pub ActiveModule);

#[derive(Resource)]
pub struct UiState {
    pub active_module: ActiveModule,
    pub frequency_hz: f32,
    pub amplitude: f32,
    pub drag: f32,
    pub restitution: f32,
    pub coupling_k: f32,
    pub fluid_density: f32,
    pub fluid_surface_tension: f32,
    pub fluid_depth: f32,
    pub fluid_damping: f32,
    pub voice_control: bool,
    pub voice_freq: f32,
    pub voice_rms: f32,
    pub fps: f32,
}

impl Default for UiState {
    fn default() -> Self {
        // Single source of truth: physical defaults come from the sim module
        // configs, so the UI and the solver can never drift apart.
        let p = sim::SimParams::default();
        Self {
            active_module: ActiveModule::Granular,
            frequency_hz: p.frequency_hz,
            amplitude: p.amplitude,
            drag: p.drag,
            restitution: p.restitution,
            coupling_k: p.coupling_k,
            fluid_density: p.fluid_density,
            fluid_surface_tension: p.fluid_surface_tension,
            fluid_depth: p.fluid_depth,
            fluid_damping: p.fluid_damping,
            voice_control: false,
            voice_freq: 0.0,
            voice_rms: 0.0,
            fps: 0.0,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, Resource)]
pub enum ActiveModule {
    #[default]
    Granular,
    Fluid,
    Acoustic,
}

impl ActiveModule {
    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            ActiveModule::Granular => "Granular",
            ActiveModule::Fluid => "Fluid",
            ActiveModule::Acoustic => "Acoustic",
        }
    }
}

#[derive(Resource, Default)]
pub struct SimState {
    pub granular_data: Option<Vec<cymatrox::granular::GranularData>>,
    pub acoustic_data: Option<Vec<cymatrox::acoustic::AcousticPressureNode>>,
    pub status: String,
    pub error: bool,
}

/// Latest-wins parameters inferred from [`UiState`]. Physical unit conversion
/// lives on the sim side, so this is a straight field copy.
impl From<&UiState> for sim::SimParams {
    fn from(state: &UiState) -> Self {
        Self {
            frequency_hz: state.frequency_hz,
            amplitude: state.amplitude,
            drag: state.drag,
            restitution: state.restitution,
            coupling_k: state.coupling_k,
            fluid_density: state.fluid_density,
            fluid_surface_tension: state.fluid_surface_tension,
            fluid_depth: state.fluid_depth,
            fluid_damping: state.fluid_damping,
        }
    }
}

pub const PANEL_WIDTH: f32 = ui::theme::PANEL_WIDTH;
pub const PANEL_MARGIN: f32 = ui::theme::PANEL_MARGIN;
pub const PANEL_GAP: f32 = ui::theme::PANEL_GAP;

/// Horizontal space reserved on the left for the floating control panel. The
/// 3D viewport is offset by this amount so it is never hidden behind the panel.
pub fn responsive_panel_width(window_logical_width: f32) -> f32 {
    let available = window_logical_width - PANEL_MARGIN * 2.0;
    let panel = PANEL_WIDTH.min(available.max(240.0));
    PANEL_MARGIN + panel + PANEL_GAP
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "cymatrox".into(),
                resolution: (1000u32, 700u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .insert_resource(UiState::default())
        .insert_resource(SimState::default())
        .insert_resource(PrevModule(ActiveModule::Granular))
        .insert_resource(ModuleDefaultsApplied::default())
        .add_systems(
            Startup,
            (
                setup_scene,
                start_sim,
                start_audio,
                render::granular3d::setup_granular_scene,
                render::fluid3d::setup_fluid_scene,
                render::acoustic3d::setup_acoustic_scene,
            ),
        )
        .add_systems(EguiPrimaryContextPass, ui::panel::ui_panel)
        // Fonts must be installed before egui's `begin_pass`, otherwise the
        // custom families aren't bound until the following frame.
        .add_systems(
            PreUpdate,
            ui::fonts::install_once.in_set(EguiPreUpdateSet::ProcessInput),
        )
        .add_systems(
            Update,
            (
                (poll_sim_data, poll_voice, sync_params, sync_module_switch),
                (
                    render::camera::manual_module_controls,
                    render::granular3d::update_granular_particles,
                    render::granular3d::toggle_granular_visibility,
                    render::camera::toggle_orbit_visibility,
                    render::camera::set_module_viewport,
                    render::fluid3d::zoom_fluid_scope,
                    render::fluid3d::fit_fluid_scope,
                    render::fluid3d::update_fluid_mesh,
                    render::fluid3d::toggle_fluid_visibility,
                    render::acoustic3d::update_acoustic_points,
                    render::acoustic3d::toggle_acoustic_visibility,
                    diagnose,
                ),
            )
                .chain(),
        )
        .run();
}

fn setup_scene(mut commands: Commands, mut egui_global_settings: ResMut<EguiGlobalSettings>) {
    render::camera::setup_camera(&mut commands);

    commands.spawn((
        DirectionalLight {
            // Strong key light for the lit 3D modules.
            illuminance: 20000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        // Light every layer for the lit 3D modules, except layer 2 (liquid):
        // the liquid dish uses its own low raking light so the cymatics
        // relief reads with contrast, not washed out.
        RenderLayers::from_layers(&[0, 1, 3]),
    ));
    // Low ambient so shadowed troughs stay dark and the raking highlights pop.
    commands.spawn(AmbientLight {
        color: Color::srgb(0.18, 0.2, 0.24),
        ..default()
    });

    egui_global_settings.auto_create_primary_context = false;

    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}

fn start_sim(mut commands: Commands) {
    let (data_rx, params_slot, sim_control) = sim::start_sim_thread();
    commands.insert_resource(data_rx);
    commands.insert_resource(params_slot);
    commands.insert_resource(sim_control);
}

fn start_audio(mut commands: Commands) {
    let (tx, rx) = crossbeam_channel::bounded::<audio::VoiceData>(4);
    audio::start_voice_capture(tx);
    commands.insert_resource(VoiceReceiver { rx });
}

fn poll_voice(voice_rx: Res<VoiceReceiver>, mut ui_state: ResMut<UiState>) {
    while let Ok(data) = voice_rx.rx.try_recv() {
        ui_state.voice_freq = data.frequency_hz;
        ui_state.voice_rms = data.amplitude;

        if ui_state.voice_control && data.frequency_hz.is_finite() {
            ui_state.frequency_hz = data.frequency_hz.clamp(20.0, 20_000.0);
        }
    }
}

fn poll_sim_data(data_rx: Res<sim::SimDataReceiver>, mut sim_state: ResMut<SimState>) {
    while let Ok(data) = data_rx.rx.try_recv() {
        match data {
            sim::SimData::Granular(frame) => {
                sim_state.granular_data = Some(frame);
                sim_state.error = false;
            }
            sim::SimData::Acoustic(frame) => {
                sim_state.acoustic_data = Some(frame);
                sim_state.error = false;
            }
            sim::SimData::Status(status) => {
                error!("[sim] {status}");
                sim_state.status = status;
                sim_state.error = true;
            }
        }
    }
}

fn diagnose(
    ui_state: Res<UiState>,
    fluid_cams: Query<&Camera, With<render::fluid3d::FluidCamera>>,
    fluid_mesh_q: Query<&Mesh3d, With<render::fluid3d::FluidHeightmap>>,
    meshes: Res<Assets<Mesh>>,
    mut frames: Local<u32>,
) {
    *frames += 1;
    if !(*frames).is_multiple_of(120) {
        return;
    }
    for cam in &fluid_cams {
        debug!(
            "[diag] module={:?} fluid_cam_active={} viewport={:?}",
            ui_state.active_module,
            cam.is_active,
            cam.viewport
                .as_ref()
                .map(|v| (v.physical_position, v.physical_size)),
        );
    }
    for mesh3d in &fluid_mesh_q {
        let verts = meshes
            .get(&mesh3d.0)
            .map(|m| m.count_vertices())
            .unwrap_or(0);
        debug!("[diag] fluid mesh vertices={}", verts);
    }
}

fn sync_params(ui_state: Res<UiState>, params_slot: Res<sim::SimParamsSlot>) {
    if ui_state.is_changed() {
        // Latest-wins slot: no bounded channel to overflow and no silent drop.
        params_slot.set(sim::SimParams::from(&*ui_state));
    }
}

fn sync_module_switch(
    mut ui_state: ResMut<UiState>,
    sim_control: Res<sim::SimControl>,
    mut prev: ResMut<PrevModule>,
    mut defaults: ResMut<ModuleDefaultsApplied>,
) {
    if ui_state.active_module != prev.0 {
        // Give each module a sensible driving regime the first time it is
        // opened, without clobbering settings the user later tunes.
        match ui_state.active_module {
            ActiveModule::Fluid if !defaults.fluid => {
                info!("[module] applying fluid defaults");
                ui_state.frequency_hz = 60.0;
                // The CymaScope figure is normalised, so the amplitude slider
                // only scales the rendered relief height on the Fluid view.
                ui_state.amplitude = 0.28;
                defaults.fluid = true;
            }
            ActiveModule::Acoustic if !defaults.acoustic => {
                info!("[module] applying acoustic defaults");
                // Acoustic drives a transducer velocity in the ultrasonic range.
                ui_state.frequency_hz = 24_000.0;
                ui_state.amplitude = 0.20;
                defaults.acoustic = true;
            }
            _ => {}
        }
        if let Err(crossbeam_channel::TrySendError::Disconnected(_)) =
            sim_control.switch_tx.try_send(ui_state.active_module)
        {
            error!("[ui] simulation thread disconnected while switching module");
        }
        prev.0 = ui_state.active_module;
    }
}

#[derive(Resource, Default)]
pub struct ModuleDefaultsApplied {
    pub fluid: bool,
    pub acoustic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_width_scales_then_clamps() {
        // Wide windows reserve the full floating panel (24 + 328 + 16).
        let p1000 = responsive_panel_width(1000.0);
        assert!((p1000 - 368.0).abs() < 1e-4);
        let p800 = responsive_panel_width(800.0);
        assert!((p800 - 368.0).abs() < 1e-4);
        let p500 = responsive_panel_width(500.0);
        assert!((p500 - 368.0).abs() < 1e-4);
        let p10000 = responsive_panel_width(10_000.0);
        assert!((p10000 - 368.0).abs() < 1e-4);
        // Narrow windows shrink the panel but never below 240px.
        let p300 = responsive_panel_width(300.0);
        assert!((p300 - 292.0).abs() < 1e-4);
    }
}
