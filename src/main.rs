use bevy::camera::{visibility::RenderLayers, CameraOutputMode};
use bevy::log::{debug, error, info};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy_egui::{
    egui, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext,
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
    pub fluid_data: Option<Vec<cymatrox::fluid::FluidSurfaceNode>>,
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

const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 210, 190);
const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(0, 140, 130);
const BG_DARK: egui::Color32 = egui::Color32::from_rgb(18, 18, 30);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(28, 28, 44);
const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(35, 35, 55);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(140, 140, 160);
const PANEL_WIDTH: f32 = 300.0;
const PANEL_MIN_WIDTH: f32 = 220.0;

pub fn responsive_panel_width(window_logical_width: f32) -> f32 {
    (window_logical_width * 0.30).clamp(PANEL_MIN_WIDTH, PANEL_WIDTH)
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
                render::granular2d::setup_granular_scene,
                render::fluid3d::setup_fluid_scene,
                render::acoustic3d::setup_acoustic_scene,
            ),
        )
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(
            Update,
            (
                (poll_sim_data, poll_voice, sync_params, sync_module_switch),
                (
                    render::camera::manual_module_controls,
                    render::granular2d::update_granular_particles,
                    render::granular2d::toggle_granular_visibility,
                    render::camera::toggle_orbit_visibility,
                    render::camera::set_module_viewport,
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

        if ui_state.voice_control
            && data.frequency_hz.is_finite()
            && data.amplitude.is_finite()
        {
            ui_state.frequency_hz = data.frequency_hz.clamp(20.0, 20_000.0);
            ui_state.amplitude = (data.amplitude * 0.01).clamp(0.00001, 1.0);
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
            sim::SimData::Fluid(frame) => {
                sim_state.fluid_data = Some(frame);
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
    sim_state: Res<SimState>,
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
            "[diag] module={:?} fluid_data={} fluid_cam_active={} viewport={:?}",
            ui_state.active_module,
            sim_state.fluid_data.as_ref().map(|d| d.len()).unwrap_or(0),
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
                ui_state.amplitude = 0.20; // x10 => 2.0 m/s^2 (visible Faraday regime)
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

fn styled_card(ui: &mut egui::Ui, title: &str, add_body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(BG_CARD)
        .corner_radius(8)
        .inner_margin(egui::Margin::same(12))
        .stroke(egui::Stroke::new(1.0, ACCENT_DIM.linear_multiply(0.3)))
        .show(ui, |ui| {
            ui.add_space(2.0);
            ui.colored_label(ACCENT, title);
            ui.add_space(4.0);
            add_body(ui);
            ui.add_space(2.0);
        });
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    text: &str,
    logarithmic: bool,
) {
    ui.label(egui::RichText::new(label).color(TEXT_DIM).size(11.0));
    let mut slider = egui::Slider::new(value, range).text(text);
    if logarithmic {
        slider = slider.logarithmic(true);
    }
    ui.add(slider);
}

fn ui_panel(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
    sim_state: Res<SimState>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) -> Result {
    if let Some(fps) = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        ui_state.fps = fps as f32;
    }

    let ctx = contexts.ctx_mut()?;
    let panel_width = responsive_panel_width(ctx.viewport_rect().width());

    // Dark theme
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = BG_DARK;
        style.visuals.window_fill = BG_DARK;
        style.visuals.widgets.noninteractive.weak_bg_fill = BG_DARK;
        style.visuals.widgets.noninteractive.bg_fill = BG_CARD;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_DIM);
        style.visuals.widgets.inactive.weak_bg_fill = BG_CARD;
        style.visuals.widgets.inactive.bg_fill = BG_CARD_HOVER;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.inactive.weak_bg_fill = BG_CARD;
        style.visuals.widgets.hovered.weak_bg_fill = ACCENT_DIM;
        style.visuals.widgets.hovered.bg_fill = ACCENT_DIM;
        style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.4);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(10.0, 7.0);
    });

    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::left("controls")
        .resizable(false)
        .default_size(panel_width)
        .show(&mut viewport_ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    // Title
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("CYMATROX")
                                .color(ACCENT)
                                .size(24.0)
                                .strong(),
                        );
                    });
                    ui.label(
                        egui::RichText::new("Interactive cymatics laboratory")
                            .color(TEXT_DIM)
                            .size(11.0),
                    );
                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Fatal initialization / solver error banner.
                    if sim_state.error {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(70, 24, 24))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::same(10))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(214, 84, 84)))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("GPU / simulation error")
                                        .color(egui::Color32::from_rgb(255, 160, 160))
                                        .strong(),
                                );
                                if !sim_state.status.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&sim_state.status)
                                            .color(egui::Color32::from_rgb(240, 200, 200))
                                            .size(11.0),
                                    );
                                }
                            });
                        ui.add_space(6.0);
                    }

                    // Module selector
                    styled_card(ui, "MODULE", |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Active: {}",
                                ui_state.active_module.label()
                            ))
                            .color(TEXT_DIM)
                            .size(11.0),
                        );
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            for (module, label) in [
                                (ActiveModule::Granular, "Granular"),
                                (ActiveModule::Fluid, "Fluid"),
                                (ActiveModule::Acoustic, "Acoustic"),
                            ] {
                                let selected = ui_state.active_module == module;
                                let btn = egui::Button::new(
                                    egui::RichText::new(label)
                                        .color(if selected { BG_DARK } else { TEXT_DIM })
                                        .size(12.0),
                                )
                                .fill(if selected { ACCENT } else { BG_CARD })
                                .min_size(egui::vec2((ui.available_width() - 16.0) / 3.0, 30.0))
                                .corner_radius(6.0);

                                if ui.add(btn).clicked() {
                                    ui_state.active_module = module;
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Driving (always visible)
                    styled_card(ui, "DRIVING", |ui| {
                        slider_row(
                            ui,
                            "Frequency",
                            &mut ui_state.frequency_hz,
                            20.0..=20_000.0,
                            "Hz",
                            true,
                        );
                        match ui_state.active_module {
                            ActiveModule::Fluid => {
                                slider_row(
                                    ui,
                                    "Amplitude",
                                    &mut ui_state.amplitude,
                                    0.0..=1.0,
                                    "×10 m/s²",
                                    false,
                                );
                            }
                            ActiveModule::Acoustic => {
                                slider_row(
                                    ui,
                                    "Amplitude",
                                    &mut ui_state.amplitude,
                                    0.0..=1.0,
                                    "×0.002 m/s",
                                    false,
                                );
                            }
                            _ => {
                                slider_row(
                                    ui,
                                    "Amplitude",
                                    &mut ui_state.amplitude,
                                    0.0..=1.0,
                                    "×1 mm",
                                    false,
                                );
                            }
                        }
                    });

                    ui.add_space(6.0);

                    // Solver (granular only, collapsible)
                    if ui_state.active_module == ActiveModule::Granular {
                        egui::CollapsingHeader::new(
                            egui::RichText::new("SOLVER").color(ACCENT).size(13.0),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::Frame::new()
                                .fill(BG_CARD)
                                .corner_radius(8)
                                .inner_margin(egui::Margin::same(12))
                                .stroke(egui::Stroke::new(1.0, ACCENT_DIM.linear_multiply(0.3)))
                                .show(ui, |ui| {
                                    slider_row(
                                        ui,
                                        "Drag",
                                        &mut ui_state.drag,
                                        0.0..=20.0,
                                        "",
                                        false,
                                    );
                                    slider_row(
                                        ui,
                                        "Restitution",
                                        &mut ui_state.restitution,
                                        0.0..=1.0,
                                        "",
                                        false,
                                    );
                                    slider_row(
                                        ui,
                                        "Coupling",
                                        &mut ui_state.coupling_k,
                                        1.0..=1.0e7,
                                        "N/m",
                                        true,
                                    );
                                });
                        });
                        ui.add_space(6.0);
                    }

                    // Liquid (fluid only, collapsible)
                    if ui_state.active_module == ActiveModule::Fluid {
                        egui::CollapsingHeader::new(
                            egui::RichText::new("LIQUID").color(ACCENT).size(13.0),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::Frame::new()
                                .fill(BG_CARD)
                                .corner_radius(8)
                                .inner_margin(egui::Margin::same(12))
                                .stroke(egui::Stroke::new(1.0, ACCENT_DIM.linear_multiply(0.3)))
                                .show(ui, |ui| {
                                    slider_row(
                                        ui,
                                        "Density",
                                        &mut ui_state.fluid_density,
                                        800.0..=1500.0,
                                        "kg/m³",
                                        false,
                                    );
                                    slider_row(
                                        ui,
                                        "Surface tension",
                                        &mut ui_state.fluid_surface_tension,
                                        0.01..=0.5,
                                        "N/m",
                                        false,
                                    );
                                    slider_row(
                                        ui,
                                        "Depth",
                                        &mut ui_state.fluid_depth,
                                        0.001..=0.02,
                                        "m",
                                        false,
                                    );
                                    slider_row(
                                        ui,
                                        "Damping",
                                        &mut ui_state.fluid_damping,
                                        0.0..=2.0,
                                        "",
                                        false,
                                    );
                                });
                        });
                        ui.add_space(6.0);
                    }

                    // Voice control
                    styled_card(ui, "VOICE", |ui| {
                        ui.checkbox(&mut ui_state.voice_control, "Enable mic input");
                        if ui_state.voice_control {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!("Freq: {:.0} Hz", ui_state.voice_freq))
                                    .color(ACCENT)
                                    .size(11.0),
                            );
                            ui.label(
                                egui::RichText::new(format!("RMS:  {:.4}", ui_state.voice_rms))
                                    .color(ACCENT)
                                    .size(11.0),
                            );
                        }
                    });

                    ui.add_space(6.0);

                    // Status
                    styled_card(ui, "STATUS", |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{:.0} FPS", ui_state.fps))
                                    .color(TEXT_DIM)
                                    .size(11.0),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let dot_color = if sim_state.granular_data.is_some()
                                        || sim_state.fluid_data.is_some()
                                        || sim_state.acoustic_data.is_some()
                                    {
                                        ACCENT
                                    } else {
                                        egui::Color32::from_rgb(242, 166, 90)
                                    };
                                    ui.colored_label(
                                        dot_color,
                                        if dot_color == ACCENT {
                                            "STREAMING"
                                        } else {
                                            "STARTING"
                                        },
                                    );
                                    ui.colored_label(dot_color, "●");
                                },
                            );
                        });
                        ui.label(
                            egui::RichText::new(if sim_state.status.is_empty() {
                                format!("{} module", ui_state.active_module.label())
                            } else {
                                sim_state.status.clone()
                            })
                            .color(ACCENT)
                            .size(12.0)
                            .strong(),
                        );

                        match ui_state.active_module {
                            ActiveModule::Granular => {
                                if let Some(data) = &sim_state.granular_data {
                                    ui.label(
                                        egui::RichText::new(format!("{} grains", data.len()))
                                            .color(TEXT_DIM)
                                            .size(11.0),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("waiting...")
                                            .color(egui::Color32::from_rgb(242, 166, 90))
                                            .size(11.0),
                                    );
                                }
                            }
                            ActiveModule::Fluid => {
                                if let Some(data) = &sim_state.fluid_data {
                                    ui.label(
                                        egui::RichText::new(format!("{} nodes", data.len()))
                                            .color(TEXT_DIM)
                                            .size(11.0),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("waiting...")
                                            .color(egui::Color32::from_rgb(242, 166, 90))
                                            .size(11.0),
                                    );
                                }
                            }
                            ActiveModule::Acoustic => {
                                if let Some(data) = &sim_state.acoustic_data {
                                    ui.label(
                                        egui::RichText::new(format!("{} cells", data.len()))
                                            .color(TEXT_DIM)
                                            .size(11.0),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("waiting...")
                                            .color(egui::Color32::from_rgb(242, 166, 90))
                                            .size(11.0),
                                    );
                                }
                            }
                        }
                    });

                    ui.add_space(8.0);
                });
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_width_scales_then_clamps() {
        let p1000 = responsive_panel_width(1000.0);
        assert!((p1000 - 300.0).abs() < 1e-4);
        let p800 = responsive_panel_width(800.0);
        assert!((p800 - 240.0).abs() < 1e-4);
        let p500 = responsive_panel_width(500.0);
        assert!((p500 - 220.0).abs() < 1e-4);
        let p10000 = responsive_panel_width(10_000.0);
        assert!((p10000 - 300.0).abs() < 1e-4);
    }
}
