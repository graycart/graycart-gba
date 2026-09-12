//! Thin egui chrome — menu + empty ROM launcher (GBA + 8-bit).
//!
//! Cited: graycart-gb `src/frontend/ui` posture (lean subset)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend/ui
//! Note: P12 — shipping GBA + DMG/CGB host (supersede cutover).

use egui::Ui;

/// Actions raised by the menu / empty-state UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    OpenRomDialog,
    TogglePause,
    Reset,
    Quit,
}

/// Top menu bar drawn inside the app `Ui`. Returns queued actions.
pub fn menu_bar(ui: &mut Ui, rom_loaded: bool, paused: bool) -> Vec<UiAction> {
    let mut actions = Vec::new();
    egui::Panel::top("menu_bar").show_inside(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open ROM…").clicked() {
                    actions.push(UiAction::OpenRomDialog);
                    ui.close();
                }
                if ui.button("Quit").clicked() {
                    actions.push(UiAction::Quit);
                    ui.close();
                }
            });
            ui.menu_button("Emulation", |ui| {
                let pause_label = if paused { "Resume" } else { "Pause" };
                if ui
                    .add_enabled(rom_loaded, egui::Button::new(pause_label))
                    .clicked()
                {
                    actions.push(UiAction::TogglePause);
                    ui.close();
                }
                if ui
                    .add_enabled(rom_loaded, egui::Button::new("Reset"))
                    .clicked()
                {
                    actions.push(UiAction::Reset);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label(format!("graycart-gba {}", env!("CARGO_PKG_VERSION")));
                ui.label("Long-term host for GBA + DMG/CGB (P12 cutover).");
            });
        });
    });
    actions
}

/// Empty-state launcher when no cartridge is loaded.
pub fn empty_rom_screen(ui: &mut Ui, actions: &mut Vec<UiAction>) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("graycart-gba");
        ui.label("Open a ROM: .gba (native) or .gb / .gbc (compat).");
        ui.add_space(8.0);
        ui.label("Recommended play host for 8-bit and GBA cartridges.");
        ui.add_space(16.0);
        if ui.button("Open ROM…").clicked() {
            actions.push(UiAction::OpenRomDialog);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_action_variants_exist() {
        let _ = UiAction::OpenRomDialog;
        let _ = UiAction::TogglePause;
        let _ = UiAction::Reset;
        let _ = UiAction::Quit;
    }
}
