//! Empty-state ROM launcher (shown when no cartridge is loaded).

use super::super::settings::FrontendSettings;
use super::UiAction;
use egui::Ui;

pub fn empty_rom_screen(ui: &mut Ui, settings: &FrontendSettings, actions: &mut Vec<UiAction>) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.heading("No ROM Loaded");
        ui.label("Open a Game Boy cartridge to start playing.");
        ui.add_space(16.0);
        if ui.button("Open ROM…").clicked() {
            actions.push(UiAction::OpenRomDialog);
        }
        ui.add_space(24.0);
        if !settings.recent.is_empty() {
            ui.label("Recent games");
            ui.add_space(8.0);
            for entry in &settings.recent {
                if ui.button(entry.menu_label()).clicked() {
                    actions.push(UiAction::LoadRom(entry.path.clone()));
                }
            }
        }
    });
}
