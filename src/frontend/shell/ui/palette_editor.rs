//! Live custom 4-shade palette editor (frontend presentation only).

use super::super::settings::FrontendSettings;
use super::super::video::PalettePreset;
use egui::Ui;

pub fn palette_editor_window(
    ctx: &egui::Context,
    settings: &mut FrontendSettings,
    open: &mut bool,
) {
    egui::Window::new("Custom Palette")
        .open(open)
        .resizable(false)
        .show(ctx, |ui| {
            palette_editor_body(ui, settings);
        });
}

fn palette_editor_body(ui: &mut Ui, settings: &mut FrontendSettings) {
    ui.label("Shade → RGB (live)");
    ui.add_space(6.0);
    let labels = [
        "Shade 0 (lightest)",
        "Shade 1",
        "Shade 2",
        "Shade 3 (darkest)",
    ];
    let mut changed = false;
    for (i, label) in labels.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(*label);
            if ui
                .color_edit_button_srgb(&mut settings.custom_colors[i])
                .changed()
            {
                changed = true;
            }
        });
    }
    ui.add_space(8.0);
    if ui.button("Reset to Classic DMG").clicked() {
        settings.reset_custom_palette();
        changed = true;
    }
    if changed {
        settings.palette = PalettePreset::Custom;
        settings.save();
    }
}
