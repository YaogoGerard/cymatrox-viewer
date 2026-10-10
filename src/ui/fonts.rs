//! Embedded typefaces from the design mock-up: **DM Sans** for UI text and
//! **DM Mono** for numeric readouts.
//!
//! The mock-up ships only the Latin subsets of these fonts, so we embed the
//! same subsets (converted from `.woff2` to `.ttf`) and keep egui's default
//! fonts as glyph fallbacks. Both families are licensed under the SIL Open
//! Font License 1.1 — see `fonts/OFL.txt`.

use bevy::ecs::system::Local;
use bevy_egui::egui;
use bevy_egui::EguiContexts;
use std::sync::Arc;

/// Key of the DM Sans proportional face.
pub const SANS: &str = "dm-sans";
/// Key of the DM Mono 400 face.
pub const MONO: &str = "dm-mono";
/// Key of the DM Mono 500 face (used by the large readout).
pub const MONO_MEDIUM: &str = "dm-mono-medium";

/// Family for the DM Mono 500 numeric readouts (frequency, etc.).
pub fn mono_medium_family() -> egui::FontFamily {
    egui::FontFamily::Name(MONO_MEDIUM.into())
}

/// Installs the embedded fonts into `ctx`. Call once, before the first frame.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        SANS.to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "fonts/DMSans-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        MONO.to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "fonts/DMMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        MONO_MEDIUM.to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "fonts/DMMono-Medium.ttf"
        ))),
    );

    // DM Sans takes priority for proportional text; egui's defaults stay as
    // fallbacks for any glyph outside the Latin subset.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, SANS.to_owned());

    // DM Mono takes priority for monospace text.
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, MONO.to_owned());

    // A dedicated family for the medium-weight numeric readout.
    fonts.families.insert(
        mono_medium_family(),
        vec![MONO_MEDIUM.to_owned(), MONO.to_owned()],
    );

    ctx.set_fonts(fonts);
}

/// Bevy system: installs the fonts exactly once, during `PreUpdate` and before
/// egui's `begin_pass`, so the custom families are bound from the first frame.
pub fn install_once(mut contexts: EguiContexts, mut installed: Local<bool>) {
    if *installed {
        return;
    }
    if let Ok(ctx) = contexts.ctx_mut() {
        install(ctx);
        *installed = true;
    }
}
