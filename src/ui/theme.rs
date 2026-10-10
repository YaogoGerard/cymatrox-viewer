use bevy_egui::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 210, 190);
pub const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(0, 140, 130);
pub const BG_DARK: egui::Color32 = egui::Color32::from_rgb(11, 11, 20);
pub const BG_CARD: egui::Color32 = egui::Color32::from_rgba_premultiplied(21, 21, 34, 224);
pub const BG_CARD_SOLID: egui::Color32 = egui::Color32::from_rgb(21, 21, 34);
pub const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(35, 35, 55);
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(36, 36, 56);
pub const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(140, 140, 160);
pub const TEXT_DIM_BRIGHT: egui::Color32 = egui::Color32::from_rgb(185, 185, 201);
pub const TEXT_NORMAL: egui::Color32 = egui::Color32::from_rgb(244, 244, 248);
pub const ERROR_BG: egui::Color32 = egui::Color32::from_rgb(70, 24, 24);
pub const ERROR_BORDER: egui::Color32 = egui::Color32::from_rgb(214, 84, 84);
pub const WARNING: egui::Color32 = egui::Color32::from_rgb(242, 166, 90);

// Floating panel geometry, mirrored from the HTML mock-up:
// `left:24px; top:24px; bottom:24px; width:328px`.
pub const PANEL_MARGIN: f32 = 24.0;
pub const PANEL_WIDTH: f32 = 328.0;
pub const PANEL_GAP: f32 = 16.0;

/// Panel fill: `rgba(21,21,34,.88)`.
pub const PANEL_BG: egui::Color32 = BG_CARD;
/// Panel border: `rgba(36,36,56,.9)` (premultiplied).
pub const PANEL_BORDER: egui::Color32 = egui::Color32::from_rgba_premultiplied(32, 32, 50, 229);

pub fn apply_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_DARK;
    visuals.window_fill = BG_DARK;
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.override_text_color = Some(TEXT_NORMAL);
    visuals.widgets.noninteractive.bg_fill = BG_DARK;
    visuals.widgets.noninteractive.weak_bg_fill = BG_DARK;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_DIM);
    visuals.widgets.inactive.bg_fill = BG_CARD_SOLID;
    visuals.widgets.inactive.weak_bg_fill = BG_CARD_SOLID;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_NORMAL);
    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_NORMAL);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT_NORMAL);
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.4);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    ctx.set_visuals(visuals);
}
