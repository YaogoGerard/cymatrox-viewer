use bevy::camera::Viewport;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

#[derive(Component)]
pub struct OrbitCamera;

#[derive(Component)]
pub struct ModuleViewportCamera;

/// Module cameras plus their per-module markers, used to route orbit input to
/// whichever 3D module is currently on screen.
type ModuleCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut PanOrbitCamera,
        &'static Camera,
        Option<&'static crate::render::fluid3d::FluidCamera>,
    ),
    With<ModuleViewportCamera>,
>;

/// Handles module cameras directly so egui's full-screen context cannot
/// prevent PanOrbit from receiving input intended for the 3D viewport.
pub fn manual_module_controls(
    ui_state: Res<crate::UiState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut cameras: ModuleCameraQuery,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let panel_width = crate::responsive_panel_width(window.width());
    if cursor.x <= panel_width {
        motion.clear();
        wheel.clear();
        return;
    }

    let delta = motion.read().map(|event| event.delta).sum::<Vec2>();
    let scroll = wheel.read().map(|event| event.y).sum::<f32>();
    let orbit = mouse.pressed(MouseButton::Left);
    let pan = mouse.pressed(MouseButton::Right);
    let zoom = mouse.pressed(MouseButton::Middle);
    if !orbit && !pan && !zoom && scroll == 0.0 {
        return;
    }

    let selected = match ui_state.active_module {
        crate::ActiveModule::Fluid => ui_state.active_module,
        crate::ActiveModule::Granular => return,
    };
    for (mut camera, _viewport_camera, fluid) in &mut cameras {
        let is_selected = match selected {
            crate::ActiveModule::Fluid => fluid.is_some(),
            crate::ActiveModule::Granular => false,
        };
        if !is_selected {
            continue;
        }
        if orbit {
            camera.target_yaw -= delta.x * 0.008;
            camera.target_pitch = (camera.target_pitch + delta.y * 0.008).clamp(-1.45, 1.45);
        } else if pan {
            let radius = camera.target_radius;
            camera.target_focus +=
                Vec3::new(delta.x * 0.006 * radius, -delta.y * 0.006 * radius, 0.0);
        } else if zoom {
            camera.target_radius = (camera.target_radius - delta.y * 0.02).clamp(0.5, 30.0);
        }
        if scroll != 0.0 {
            camera.target_radius = (camera.target_radius * (1.0 - scroll * 0.08)).clamp(0.5, 30.0);
        }
    }
}

pub fn setup_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        Transform::from_xyz(2.0, 2.0, 5.0),
        PanOrbitCamera {
            focus: Vec3::new(0.0, 0.5, 0.0),
            ..default()
        },
        OrbitCamera,
        Visibility::Hidden,
    ));
}

/// Offsets module cameras by the same responsive width used by the egui panel.
pub fn set_module_viewport(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cam_query: Query<&mut Camera, With<ModuleViewportCamera>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let scale = window.scale_factor().max(0.001);
    let logical_width = window.physical_width() as f32 / scale;
    let w = window.physical_width().max(1);
    let h = window.physical_height();
    let requested_panel_width = (crate::responsive_panel_width(logical_width) * scale) as u32;
    let panel_width_px = requested_panel_width.min(w.saturating_sub(1));
    for mut cam in &mut cam_query {
        cam.viewport = Some(Viewport {
            physical_position: UVec2::new(panel_width_px, 0),
            physical_size: UVec2::new(w - panel_width_px, h),
            ..default()
        });
    }
}

pub fn toggle_orbit_visibility(
    _ui_state: Res<crate::UiState>,
    mut query: Query<(&mut Visibility, &mut Camera), With<OrbitCamera>>,
) {
    for (mut vis, mut cam) in &mut query {
        *vis = Visibility::Hidden;
        cam.is_active = false;
    }
}
