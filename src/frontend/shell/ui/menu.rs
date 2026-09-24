//! Top menu bar — only working commands (no unfinished stubs).

use super::super::audio::{
    AudioDevicePref, list_output_device_names, should_refresh_output_device_list,
};
use super::super::playback::SpeedPreset;
use super::super::settings::FrontendSettings;
use super::super::video::{DisplayMode, PalettePreset};
use egui::Ui;
use graycart::HostHardwarePref;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum UiAction {
    OpenRomDialog,
    LoadRom(PathBuf),
    Quit,
    TogglePause,
    Reset,
    ToggleFullscreen,
    ToggleDebugMonitor,
    ToggleControls,
    QuickSave,
    QuickLoad,
    SaveSlot(u8),
    LoadSlot(u8),
    TakeScreenshot,
    InstallBootRom,
    ClearBootRom,
    SetFfSpeed(SpeedPreset),
    SetHardwarePref(graycart::HostHardwarePref),
    SetAudioOutput(crate::frontend::shell::audio::AudioDevicePref),
    ReportBug,
    RequestFeature,
    ConfigureGithubToken,
}

#[derive(Debug, Default)]
pub struct RuntimeUi {
    pub paused: bool,
    pub ff_toggle: bool,
    pub show_palette_editor: bool,
    pub rom_loaded: bool,
    /// True while the running machine is ARM (`.gba`). Hides DMG palette menus.
    pub is_arm: bool,
    pub debug_monitor_open: bool,
    pub controls_open: bool,
    pub status_toast: Option<String>,
    /// Playback overlay (egui chrome; excluded from F6 screenshots).
    pub overlay_rewinding: bool,
    pub overlay_paused: bool,
    pub overlay_frame_advance: bool,
    pub overlay_speed: Option<SpeedPreset>,
    /// Cached WASAPI/CPAL names for the Audio menu (enumeration is not per-frame).
    output_device_names: Vec<String>,
    output_devices_at: Option<Instant>,
}

const OUTPUT_DEVICE_LIST_TTL: Duration = Duration::from_secs(2);

fn cached_output_device_names(runtime: &mut RuntimeUi) -> &[String] {
    let age = runtime.output_devices_at.map(|t| t.elapsed());
    if should_refresh_output_device_list(age, OUTPUT_DEVICE_LIST_TTL) {
        runtime.output_device_names = list_output_device_names();
        runtime.output_devices_at = Some(Instant::now());
    }
    &runtime.output_device_names
}

pub fn menu_bar(
    ui: &mut Ui,
    settings: &mut FrontendSettings,
    runtime: &mut RuntimeUi,
    actions: &mut Vec<UiAction>,
) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open ROM…").clicked() {
                actions.push(UiAction::OpenRomDialog);
                ui.close();
            }
            ui.menu_button("Recent ROMs", |ui| {
                if settings.recent.is_empty() {
                    ui.label("(none yet)");
                } else {
                    let recents = settings.recent.clone();
                    for entry in recents {
                        if ui.button(entry.menu_label()).clicked() {
                            actions.push(UiAction::LoadRom(entry.path));
                            ui.close();
                        }
                    }
                }
            });
            ui.separator();
            if ui.button("Quit").clicked() {
                actions.push(UiAction::Quit);
                ui.close();
            }
        });

        ui.menu_button("Emulation", |ui| {
            if runtime.rom_loaded {
                let pause_label = if runtime.paused { "Resume" } else { "Pause" };
                if ui.button(pause_label).clicked() {
                    actions.push(UiAction::TogglePause);
                    ui.close();
                }
                if ui.button("Reset").clicked() {
                    actions.push(UiAction::Reset);
                    ui.close();
                }
                if ui.button("Quick Save").clicked() {
                    actions.push(UiAction::QuickSave);
                    ui.close();
                }
                if ui.button("Quick Load").clicked() {
                    actions.push(UiAction::QuickLoad);
                    ui.close();
                }
                if ui.button("Screenshot").clicked() {
                    actions.push(UiAction::TakeScreenshot);
                    ui.close();
                }
                ui.menu_button("Save Slot", |ui| {
                    for slot in 0..10u8 {
                        if ui.button(format!("{slot}")).clicked() {
                            actions.push(UiAction::SaveSlot(slot));
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Load Slot", |ui| {
                    for slot in 0..10u8 {
                        if ui.button(format!("{slot}")).clicked() {
                            actions.push(UiAction::LoadSlot(slot));
                            ui.close();
                        }
                    }
                });
                if ui
                    .checkbox(&mut settings.rewind_enabled, "Rewind")
                    .changed()
                {
                    settings.save();
                }
                ui.checkbox(&mut runtime.ff_toggle, "Fast Forward");
                ui.menu_button("Speed", |ui| {
                    for preset in SpeedPreset::ALL {
                        let selected = settings.ff_speed == preset;
                        if ui.selectable_label(selected, preset.label()).clicked() {
                            actions.push(UiAction::SetFfSpeed(preset));
                            ui.close();
                        }
                    }
                });
            }
            if ui
                .checkbox(&mut settings.pause_when_unfocused, "Pause When Unfocused")
                .changed()
            {
                settings.save();
            }
            if ui.checkbox(&mut settings.skip_boot, "Skip Boot").changed() {
                settings.save();
            }
            if ui.button("Install Boot ROM…").clicked() {
                actions.push(UiAction::InstallBootRom);
                ui.close();
            }
            if ui.button("Clear Boot ROM").clicked() {
                actions.push(UiAction::ClearBootRom);
                ui.close();
            }
            ui.separator();
            ui.menu_button("Hardware", |ui| {
                for pref in HostHardwarePref::ALL {
                    let selected = settings.hardware_pref == pref;
                    if ui.selectable_label(selected, pref.menu_label()).clicked() {
                        actions.push(UiAction::SetHardwarePref(pref));
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button("Video", |ui| {
            if runtime.rom_loaded && !runtime.is_arm {
                ui.menu_button("Palette", |ui| {
                    for preset in PalettePreset::ALL {
                        let selected = settings.palette == preset;
                        if ui.selectable_label(selected, preset.label()).clicked() {
                            settings.palette = preset;
                            if preset == PalettePreset::Custom {
                                runtime.show_palette_editor = true;
                            }
                            settings.save();
                            ui.close();
                        }
                    }
                });
                if ui.button("Custom Palette…").clicked() {
                    settings.palette = PalettePreset::Custom;
                    runtime.show_palette_editor = true;
                    settings.save();
                    ui.close();
                }
            }
            ui.menu_button("Display", |ui| {
                for mode in DisplayMode::ALL {
                    let selected = settings.display_mode == mode;
                    if ui.selectable_label(selected, mode.label()).clicked() {
                        settings.display_mode = mode;
                        settings.save();
                        ui.close();
                    }
                }
            });
            if ui
                .checkbox(&mut settings.integer_scaling, "Integer Scaling")
                .changed()
            {
                settings.save();
            }
            if ui.button("Fullscreen").clicked() {
                actions.push(UiAction::ToggleFullscreen);
                ui.close();
            }
        });

        ui.menu_button("Input", |ui| {
            let label = if runtime.controls_open {
                "Close Controls"
            } else {
                "Configure Controls…"
            };
            if ui.button(label).clicked() {
                actions.push(UiAction::ToggleControls);
                ui.close();
            }
        });

        ui.menu_button("Audio", |ui| {
            let mut vol = settings.volume_percent;
            if ui
                .add(egui::Slider::new(&mut vol, 0..=100).text("Master Volume"))
                .changed()
            {
                settings.volume_percent = vol;
                settings.save();
            }
            if ui.checkbox(&mut settings.muted, "Mute").changed() {
                settings.save();
            }
            ui.separator();
            if ui
                .selectable_label(
                    matches!(settings.audio_output, AudioDevicePref::SystemDefault),
                    "Output: System Default",
                )
                .clicked()
            {
                actions.push(UiAction::SetAudioOutput(AudioDevicePref::SystemDefault));
                ui.close();
            }
            let names = cached_output_device_names(runtime).to_vec();
            for name in names {
                let selected = matches!(
                    &settings.audio_output,
                    AudioDevicePref::Device { name: n } if *n == name
                );
                if ui
                    .selectable_label(selected, format!("Output: {name}"))
                    .clicked()
                {
                    actions.push(UiAction::SetAudioOutput(AudioDevicePref::Device { name }));
                    ui.close();
                }
            }
        });

        ui.menu_button("Help", |ui| {
            ui.label(format!(
                "{} {}",
                super::super::brand::NAME,
                super::super::brand::crate_version()
            ));
            ui.separator();
            if ui.button("Report bug…").clicked() {
                actions.push(UiAction::ReportBug);
                ui.close();
            }
            if ui.button("Request feature…").clicked() {
                actions.push(UiAction::RequestFeature);
                ui.close();
            }
            if ui.button("Optional GitHub token…").clicked() {
                actions.push(UiAction::ConfigureGithubToken);
                ui.close();
            }
        });

        ui.menu_button("Debug", |ui| {
            let label = if runtime.debug_monitor_open {
                "Close Monitor"
            } else {
                "Open Monitor"
            };
            if ui.button(label).clicked() {
                actions.push(UiAction::ToggleDebugMonitor);
                ui.close();
            }
        });

        if runtime.paused {
            ui.separator();
            ui.label("Paused");
        }
        if let Some(msg) = &runtime.status_toast {
            ui.separator();
            ui.label(msg);
        }
    });
}
