use bevy_egui::egui;

use super::theme::{ACCENT, BG_DARK, BORDER, TEXT_DIM, TEXT_DIM_BRIGHT, TEXT_NORMAL};

const TRACK_HEIGHT: f32 = 4.0;
const KNOB_RADIUS: f32 = 7.0;

/// Small uppercase section header: `10px`, `letter-spacing:.12em`, `#8C8CA0`.
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(TEXT_DIM)
            .size(10.0)
            .extra_letter_spacing(1.2),
    );
}

/// Full-width hairline: `height:1px; background:#242438`.
pub fn divider(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
}

/// Large monospace readout with a dim unit suffix: `38px` value, `14px` unit.
/// Rendered manually so the row keeps the design's `line-height:1` (38px).
pub fn big_readout(ui: &mut egui::Ui, value: &str, unit: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::hover());
    let val = ui.painter().layout_no_wrap(
        value.to_owned(),
        egui::FontId::new(38.0, super::fonts::mono_medium_family()),
        TEXT_NORMAL,
    );
    let unit_gal =
        ui.painter()
            .layout_no_wrap(unit.to_owned(), egui::FontId::monospace(14.0), TEXT_DIM);
    let val_pos = egui::pos2(rect.left(), rect.center().y - val.size().y * 0.5);
    let unit_pos = egui::pos2(
        rect.left() + val.size().x + 6.0,
        val_pos.y + val.size().y - unit_gal.size().y - 2.0,
    );
    let painter = ui.painter();
    painter.galley(val_pos, val, TEXT_NORMAL);
    painter.galley(unit_pos, unit_gal, TEXT_DIM);
}

/// A labelled slider rendered exactly like the mock-up: a label/value row over a
/// thin 4px track with a filled portion and a round knob. When `logarithmic` is
/// true the value is mapped on a log10 scale (used by the frequency slider).
#[allow(clippy::too_many_arguments)]
pub fn slider_row(
    ui: &mut egui::Ui,
    label: Option<&str>,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    value_text: &str,
    value_unit: &str,
    fill: egui::Color32,
    knob: egui::Color32,
    label_gap: f32,
    logarithmic: bool,
) -> bool {
    let mut changed = false;
    let (min, max) = (*range.start(), *range.end());

    let pos_to_value = |t: f32| -> f32 {
        if logarithmic {
            let lo = min.max(f32::MIN_POSITIVE).log10();
            let hi = max.max(f32::MIN_POSITIVE).log10();
            10.0f32.powf(lo + t * (hi - lo))
        } else {
            min + t * (max - min)
        }
    };
    let value_to_t = |v: f32| -> f32 {
        if logarithmic {
            let lo = min.max(f32::MIN_POSITIVE).log10();
            let hi = max.max(f32::MIN_POSITIVE).log10();
            let vv = v.max(f32::MIN_POSITIVE).log10();
            ((vv - lo) / (hi - lo)).clamp(0.0, 1.0)
        } else if (max - min).abs() > f32::EPSILON {
            ((v - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    if let Some(label) = label {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(TEXT_DIM_BRIGHT).size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !value_unit.is_empty() {
                    ui.label(
                        egui::RichText::new(value_unit)
                            .color(TEXT_DIM)
                            .size(12.0)
                            .monospace(),
                    );
                }
                ui.label(
                    egui::RichText::new(value_text)
                        .color(TEXT_NORMAL)
                        .size(12.0)
                        .monospace(),
                );
            });
        });
        ui.add_space(label_gap);
    }

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), TRACK_HEIGHT),
        egui::Sense::hover(),
    );
    let hit = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(rect.width(), KNOB_RADIUS * 2.0 + 4.0),
    );
    let response = ui.interact(
        hit,
        ui.id().with(("slider", label.unwrap_or("frequency"))),
        egui::Sense::click_and_drag(),
    );

    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
            let new = pos_to_value(t);
            if (new - *value).abs() > f32::EPSILON {
                *value = new;
                changed = true;
            }
        }
    }

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let t = value_to_t(*value);

    let cy = rect.center().y;
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left(), cy - TRACK_HEIGHT * 0.5),
        egui::pos2(rect.right(), cy + TRACK_HEIGHT * 0.5),
    );
    let knob_x = track.left() + t * track.width();

    let painter = ui.painter();
    painter.rect_filled(track, egui::CornerRadius::same(2), BORDER);
    let filled = egui::Rect::from_min_max(track.min, egui::pos2(knob_x, track.max.y));
    painter.rect_filled(filled, egui::CornerRadius::same(2), fill);
    painter.circle_filled(egui::pos2(knob_x, cy), KNOB_RADIUS, knob);

    changed
}

/// Segmented module selector: a `#0B0B14` well with a filled accent tab.
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let inner = rect.shrink(4.0);
    let gap = 4.0;
    let count = labels.len().max(1) as f32;
    let tab_width = ((inner.width() - gap * (count - 1.0)) / count).max(1.0);
    let step = tab_width + gap;

    let mut clicked = None;
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let index = ((pos.x - inner.left()) / step).floor();
            if index >= 0.0 && (index as usize) < labels.len() {
                clicked = Some(index as usize);
            }
        }
    }

    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(10), BG_DARK);
    for (i, label) in labels.iter().enumerate() {
        let min = egui::pos2(inner.left() + step * i as f32, inner.top());
        let tab = egui::Rect::from_min_max(min, egui::pos2(min.x + tab_width, inner.bottom()));
        let (text_color, bg) = if i == selected {
            (BG_DARK, ACCENT)
        } else {
            (TEXT_DIM_BRIGHT, egui::Color32::TRANSPARENT)
        };
        painter.rect_filled(tab, egui::CornerRadius::same(7), bg);
        painter.text(
            tab.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(12.0),
            text_color,
        );
    }

    clicked
}

/// iOS-style switch: 38x22 track, 16x16 knob.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool) -> bool {
    let mut changed = false;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(38.0, 22.0), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        changed = true;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // On: accent track, near-black knob. Off: border track, dim grey knob.
    let (bg, knob) = if *on {
        (ACCENT, BG_DARK)
    } else {
        (BORDER, TEXT_DIM)
    };
    let cx = if *on {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(11), bg);
    painter.circle_filled(egui::pos2(cx, rect.center().y), 8.0, knob);

    changed
}

/// Status chip with a tinted background and a leading dot.
pub fn status_pill(ui: &mut egui::Ui, text: &str, color: egui::Color32, alpha: f32) {
    egui::Frame::new()
        .fill(tint(color, alpha))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(11, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                let (dot, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, color);
                ui.label(
                    egui::RichText::new(text)
                        .color(color)
                        .size(10.0)
                        .strong()
                        .extra_letter_spacing(1.0),
                );
            });
        });
}

fn tint(color: egui::Color32, alpha: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha * 255.0).round() as u8,
    )
}
