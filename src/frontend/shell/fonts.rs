//! Shared egui font setup (Departure Mono for all host UI).

use egui::{Context, FontData, FontDefinitions, FontFamily};

const DEPARTURE_MONO: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");

/// Install Departure Mono as the primary proportional and monospace face.
///
/// Used by the main game window (menus / palette / empty ROM), Machine Monitor,
/// and Configure Controls so every egui surface shares the same typeface.
pub fn install_departure_mono(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "DepartureMono".to_owned(),
        std::sync::Arc::new(FontData::from_static(DEPARTURE_MONO)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "DepartureMono".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "DepartureMono".to_owned());
    ctx.set_fonts(fonts);
}
