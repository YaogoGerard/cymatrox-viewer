use super::theme::{
    ACCENT, ACCENT_DIM, BG_DARK, ERROR_BG, ERROR_BORDER, PANEL_BG, PANEL_BORDER, PANEL_GAP,
    PANEL_MARGIN, TEXT_DIM, TEXT_DIM_BRIGHT, TEXT_NORMAL, WARNING,
};
use super::widgets;
use crate::{
    render::fluid3d,
    render::granular3d::PLATE_HALF_PX,
    sim::{FLUID_AMPLITUDE_MPS2, GRANULAR_AMPLITUDE_M},
    ActiveModule, SimState, UiState,
};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::EguiContexts;

pub fn ui_panel(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
    sim_state: Res<SimState>,
    fluid_zoom: Res<fluid3d::FluidZoom>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) -> Result {
    if let Some(fps) = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        ui_state.fps = fps as f32;
    }

    let ctx = contexts.ctx_mut()?;
    super::theme::apply_dark_theme(ctx);

    let viewport = ctx.viewport_rect();
    let total = crate::responsive_panel_width(viewport.width());
    let panel_width = (total - PANEL_MARGIN - PANEL_GAP).max(240.0);
    let panel_height = (viewport.height() - PANEL_MARGIN * 2.0).max(240.0);

    egui::Area::new("control_panel".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(PANEL_MARGIN, PANEL_MARGIN))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANEL_BG)
                .corner_radius(16)
                .stroke(egui::Stroke::new(1.0, PANEL_BORDER))
                .inner_margin(egui::Margin::symmetric(20, 22))
                .show(ui, |ui| {
                    ui.set_width(panel_width - 40.0);
                    ui.set_height(panel_height - 44.0);
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    if contents(ui, &mut ui_state, &sim_state) {
                        ui_state.set_changed();
                    }
                });
        });

    draw_frequency_ruler(
        ctx,
        ui_state.frequency_hz,
        ui_state.active_module,
        viewport,
        total,
    );
    draw_stage_caption(ctx, ui_state.active_module, viewport, total, fluid_zoom.0);

    Ok(())
}

/// Caption pinned just above the specimen in the current view. The granular
/// module presents an aluminium plate; the fluid module presents a top-down
/// cymascope pool. Each is called out beside the object itself, matching the
/// mock-up's instrument labelling.
fn draw_stage_caption(
    ctx: &egui::Context,
    active_module: ActiveModule,
    viewport: egui::Rect,
    panel_total: f32,
    fluid_zoom: f32,
) {
    // The object is centred in the module viewport (the window minus the panel
    // strip on the left). On narrow windows it can fill the whole width, so the
    // caption sits centred just above its top edge — never over the figure or
    // the frequency ruler.
    let (text, id, object_top) = match active_module {
        ActiveModule::Granular => (
            "ALUMINIUM PLATE",
            "granular-plate-caption",
            viewport.center().y - PLATE_HALF_PX,
        ),
        ActiveModule::Fluid => {
            // Mirror `fit_fluid_scope`: the instrument's outer radius occupies
            // `SCOPE_FRACTION` of the shorter viewport edge (times the zoom), and
            // the scope is lifted by `SCOPE_LIFT_FRACTION` of that radius. Put the
            // caption just above the *outer* bezel so it clears the ring, not
            // hugging the (smaller) water pool.
            let vp_left = viewport.left() + panel_total;
            let vp_w = (viewport.right() - vp_left).max(1.0);
            let vp_h = viewport.height().max(1.0);
            let min_edge = vp_w.min(vp_h);
            let scope_r_px = fluid3d::SCOPE_FRACTION * 0.5 * min_edge * fluid_zoom.max(0.0);
            let lift_px = fluid3d::SCOPE_LIFT_FRACTION * scope_r_px;
            (
                "CYMASCOPE",
                "fluid-scope-caption",
                viewport.center().y - scope_r_px - lift_px,
            )
        }
    };

    let center_x = (viewport.left() + panel_total + viewport.right()) * 0.5;
    let y = (object_top - 12.0).max(viewport.top() + 8.0);

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new(id),
    ));
    painter.text(
        egui::pos2(center_x, y),
        egui::Align2::CENTER_BOTTOM,
        text,
        egui::FontId::monospace(11.0),
        ACCENT_DIM,
    );
}

/// Frequency ruler across the bottom of the visualisation area, mirroring the
/// mock-up: a log10 scale labelled at 0% / 33.3% / 66.6% / 100% of the active
/// module's band, with a tick strip, scale labels and an accent marker (plus
/// its label) at the current frequency.
fn draw_frequency_ruler(
    ctx: &egui::Context,
    frequency_hz: f32,
    active_module: ActiveModule,
    viewport: egui::Rect,
    panel_total: f32,
) {
    // Canvas = the region the 3D module viewport occupies, minus a bottom gap.
    let canvas = egui::Rect::from_min_max(
        egui::pos2(viewport.left() + panel_total, viewport.top()),
        egui::pos2(viewport.right(), viewport.bottom() - 32.0),
    );
    let inset_left = canvas.width() * 0.11;
    let inset_right = canvas.width() * 0.14;
    let ruler = egui::Rect::from_min_max(
        egui::pos2(canvas.left() + inset_left, canvas.bottom() - 64.0),
        egui::pos2(canvas.right() - inset_right, canvas.bottom()),
    );
    if ruler.width() < 160.0 {
        return;
    }

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("frequency_ruler"),
    ));

    // Tick strip: a 1px hairline every 8px, matching the mock-up's repeating
    // gradient (`rgba(185,185,201,.35)`).
    let tick_top = ruler.top() + 30.0;
    let tick_bottom = tick_top + 12.0;
    let tick_color = TEXT_DIM_BRIGHT.gamma_multiply(0.35);
    let mut x = ruler.left();
    while x <= ruler.right() {
        painter.line_segment(
            [egui::pos2(x, tick_top), egui::pos2(x, tick_bottom)],
            egui::Stroke::new(1.0, tick_color),
        );
        x += 8.0;
    }

    // The band is audible for both remaining modules. Labels sit at the band's
    // log thirds (which land exactly on 20/200/2k/20k).
    let (fmin, fmax) = crate::frequency_range(active_module);
    let span = (fmax / fmin).max(1.0);
    let frac = |f: f32| -> f32 { ((f.max(1e-3) / fmin).log10() / span.log10()).clamp(0.0, 1.0) };
    let row_y = ruler.top() + 46.0;
    for (t, align) in [
        (0.0_f32, egui::Align2::LEFT_TOP),
        (1.0 / 3.0, egui::Align2::CENTER_TOP),
        (2.0 / 3.0, egui::Align2::CENTER_TOP),
        (1.0, egui::Align2::RIGHT_TOP),
    ] {
        painter.text(
            egui::pos2(ruler.left() + t * ruler.width(), row_y),
            align,
            frequency_tick_label(fmin * span.powf(t)),
            egui::FontId::monospace(10.0),
            TEXT_DIM,
        );
    }

    // Marker at the current frequency.
    let t = frac(frequency_hz.clamp(fmin, fmax));
    let mx = ruler.left() + t * ruler.width();
    painter.line_segment(
        [
            egui::pos2(mx, ruler.top()),
            egui::pos2(mx, ruler.bottom() - 22.0),
        ],
        egui::Stroke::new(1.0, ACCENT),
    );

    let galley = painter.layout_no_wrap(
        frequency_label(frequency_hz),
        egui::FontId::monospace(11.0),
        ACCENT,
    );
    let label_pos = egui::pos2(mx - galley.size().x * 0.5, ruler.top() - 2.0);
    painter.rect_filled(
        egui::Rect::from_min_size(label_pos, galley.size()).expand2(egui::vec2(6.0, 0.0)),
        0,
        BG_DARK,
    );
    painter.galley(label_pos, galley, ACCENT);
}

/// Formats the marker label like the mock-up: `440 Hz`, `2.5k Hz`, `24k Hz`.
fn frequency_label(frequency_hz: f32) -> String {
    if frequency_hz >= 1000.0 {
        let k = frequency_hz / 1000.0;
        if (k.round() - k).abs() < 0.05 {
            format!("{k:.0}k Hz")
        } else {
            format!("{k:.1}k Hz")
        }
    } else {
        format!("{frequency_hz:.0} Hz")
    }
}

/// Short scale label for the ruler (`20`, `200`, `2k`, `20k`, `5k`...).
fn frequency_tick_label(frequency_hz: f32) -> String {
    if frequency_hz >= 1000.0 {
        let k = frequency_hz / 1000.0;
        if (k.round() - k).abs() < 0.05 {
            format!("{k:.0}k")
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{frequency_hz:.0}")
    }
}

fn contents(ui: &mut egui::Ui, ui_state: &mut UiState, sim_state: &SimState) -> bool {
    let mut changed = false;

    // Title
    ui.label(
        egui::RichText::new("CYMATROX")
            .color(ACCENT)
            .size(22.0)
            .strong()
            .extra_letter_spacing(3.0),
    );
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new("Cymatics viewer")
            .color(TEXT_DIM)
            .size(11.0),
    );
    ui.add_space(18.0);

    // Module selector
    let selected = match ui_state.active_module {
        ActiveModule::Granular => 0,
        ActiveModule::Fluid => 1,
    };
    if let Some(index) = widgets::segmented(ui, &["Granular", "Fluid"], selected) {
        let module = match index {
            0 => ActiveModule::Granular,
            _ => ActiveModule::Fluid,
        };
        if module != ui_state.active_module {
            ui_state.active_module = module;
            changed = true;
        }
    }
    ui.add_space(18.0);

    // Error banner sits *after* the module selector, matching the mock-up.
    if sim_state.error {
        egui::Frame::new()
            .fill(ERROR_BG)
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .stroke(egui::Stroke::new(1.0, ERROR_BORDER))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("ERROR")
                        .color(ERROR_BORDER)
                        .size(10.0)
                        .strong()
                        .extra_letter_spacing(1.2),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(&sim_state.status)
                        .color(TEXT_NORMAL)
                        .size(12.0),
                );
            });
        ui.add_space(18.0);
    }

    // Control
    widgets::section_label(ui, "CONTROL");
    ui.add_space(8.0);
    let (fmin, fmax) = crate::frequency_range(ui_state.active_module);
    let freq_text = if ui_state.frequency_hz >= 1000.0 {
        format!("{:.0}", ui_state.frequency_hz)
    } else {
        format!("{:.1}", ui_state.frequency_hz)
    };
    widgets::big_readout(ui, &freq_text, "Hz");
    ui.add_space(14.0);

    let mut frequency = ui_state.frequency_hz;
    if widgets::slider_row(
        ui,
        None,
        &mut frequency,
        fmin..=fmax,
        "",
        "",
        ACCENT,
        TEXT_NORMAL,
        8.0,
        true,
    ) {
        ui_state.frequency_hz = frequency;
        changed = true;
    }
    ui.add_space(16.0);

    let unit = amplitude_unit(ui_state.active_module);
    let mut amplitude = ui_state.amplitude;
    if widgets::slider_row(
        ui,
        Some("Amplitude"),
        &mut amplitude,
        0.0..=1.0,
        &format!("{:.2}", ui_state.amplitude),
        &unit,
        ACCENT,
        TEXT_NORMAL,
        10.0,
        false,
    ) {
        ui_state.amplitude = amplitude;
        changed = true;
    }
    ui.add_space(18.0);

    widgets::divider(ui);
    ui.add_space(18.0);

    // Physics (module specific).
    match ui_state.active_module {
        ActiveModule::Granular => {
            widgets::section_label(ui, "PHYSICS · GRANULAR");
            ui.add_space(10.0);
            param_row(ui, "Friction", &mut ui_state.drag, 0.0..=20.0, 2, "");
            ui.add_space(14.0);
            param_row(
                ui,
                "Restitution",
                &mut ui_state.restitution,
                0.0..=1.0,
                2,
                "",
            );
            ui.add_space(14.0);
            param_row(
                ui,
                "Coupling k",
                &mut ui_state.coupling_k,
                1.0..=1.0e7,
                0,
                "",
            );
            ui.add_space(18.0);
            widgets::divider(ui);
            ui.add_space(18.0);
        }
        ActiveModule::Fluid => {
            widgets::section_label(ui, "PHYSICS · LIQUID");
            ui.add_space(10.0);
            param_row(
                ui,
                "Density",
                &mut ui_state.fluid_density,
                800.0..=1500.0,
                0,
                "kg/m³",
            );
            ui.add_space(14.0);
            param_row(
                ui,
                "Surface tension",
                &mut ui_state.fluid_surface_tension,
                0.01..=0.5,
                3,
                "N/m",
            );
            ui.add_space(14.0);
            param_row(ui, "Depth", &mut ui_state.fluid_depth, 0.001..=0.02, 3, "m");
            ui.add_space(14.0);
            param_row(ui, "Damping", &mut ui_state.fluid_damping, 0.0..=2.0, 2, "");
            ui.add_space(18.0);
            widgets::divider(ui);
            ui.add_space(18.0);
        }
    }

    // Voice control
    let mut voice = ui_state.voice_control;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Voice control")
                    .color(TEXT_NORMAL)
                    .size(12.0),
            );
            // The mock-up only reveals the live readout once voice control is on.
            if voice {
                ui.add_space(3.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{:.0} Hz · RMS {:.2}",
                        ui_state.voice_freq, ui_state.voice_rms
                    ))
                    .color(TEXT_DIM)
                    .size(11.0)
                    .monospace(),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            widgets::toggle(ui, &mut voice);
        });
    });
    ui_state.voice_control = voice;

    // Footer pinned to the bottom
    let remaining = ui.available_height() - 40.0;
    if remaining > 0.0 {
        ui.add_space(remaining);
    }
    widgets::divider(ui);
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        // Before the first frame the smoothed FPS is unavailable; the mock-up
        // shows an em dash rather than a misleading "0 FPS".
        let fps_text = if ui_state.fps > 0.0 {
            format!("{:.0} FPS", ui_state.fps)
        } else {
            "— FPS".to_owned()
        };
        ui.label(
            egui::RichText::new(fps_text)
                .color(TEXT_DIM_BRIGHT)
                .size(12.0)
                .monospace(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (text, color) = status(ui_state, sim_state);
            let alpha = if sim_state.error { 0.14 } else { 0.12 };
            widgets::status_pill(ui, text, color, alpha);
        });
    });

    changed
}

fn param_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
    unit: &str,
) -> bool {
    let text = format!("{:.decimals$}", *value);
    widgets::slider_row(
        ui,
        Some(label),
        value,
        range,
        &text,
        unit,
        ACCENT_DIM,
        TEXT_DIM_BRIGHT,
        8.0,
        false,
    )
}

fn amplitude_unit(module: ActiveModule) -> String {
    match module {
        ActiveModule::Granular => format!("×{:.0} mm", GRANULAR_AMPLITUDE_M * 1000.0),
        ActiveModule::Fluid => format!("×{:.0} m/s²", FLUID_AMPLITUDE_MPS2),
    }
}

fn status(ui_state: &UiState, sim_state: &SimState) -> (&'static str, egui::Color32) {
    if sim_state.error {
        return ("ERROR", ERROR_BORDER);
    }
    let streaming = match ui_state.active_module {
        ActiveModule::Granular => sim_state.granular_data.is_some(),
        // The CymaScope figure is computed synchronously on the render side
        // (analytic glyph), so the Fluid module is ready as soon as it is shown.
        ActiveModule::Fluid => true,
    };
    if streaming {
        ("STREAMING", ACCENT)
    } else {
        ("STARTING", WARNING)
    }
}
