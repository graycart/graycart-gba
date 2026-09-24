//! Configure Controls — remapping, deadzone, device selection, live tester.

use super::super::host_input::{
    GamepadMap, InputFrontend, ListenTarget, PollResult, button_bit, keycode_from_name,
    keycode_label, pad_button_label,
};
use super::super::settings::FrontendSettings;
use egui::{Color32, FontId, RichText, Ui};
use graycart::GameBoyButton;

/// Draw the controls editor into an existing egui UI (native Controls window).
pub fn draw_controls(
    ui: &mut Ui,
    settings: &mut FrontendSettings,
    input: &mut InputFrontend,
    poll: &PollResult,
) {
    ui.label(
        RichText::new("CONFIGURE CONTROLS")
            .font(FontId::monospace(13.0))
            .strong(),
    );
    ui.add_space(6.0);

    if input.is_listening() {
        ui.horizontal(|ui| {
            ui.colored_label(
                Color32::LIGHT_YELLOW,
                RichText::new("Press a key or controller button…").font(FontId::monospace(12.0)),
            );
            ui.label(RichText::new("(Escape to cancel)").font(FontId::monospace(11.0)));
        });
        ui.add_space(4.0);
    }

    if poll.frame.unavailable {
        ui.colored_label(
            Color32::LIGHT_RED,
            RichText::new("Gamepad support unavailable").font(FontId::monospace(12.0)),
        );
        ui.add_space(4.0);
    }

    device_row(ui, settings, poll);
    ui.add_space(8.0);
    gilrs_status_panel(ui, poll);
    ui.add_space(8.0);

    binding_table(ui, settings, input, poll);
    ui.add_space(8.0);

    deadzone_row(ui, settings);
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("Restore Defaults").font(FontId::monospace(12.0)))
            .clicked()
        {
            settings
                .input
                .restore_defaults(poll.frame.active_profile_id.as_deref());
            settings.save();
        }
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new("Live tester")
            .font(FontId::monospace(12.0))
            .strong(),
    );
    live_tester(ui, poll.effective_mask);
    ui.add_space(4.0);

    egui::CollapsingHeader::new(RichText::new("Details").font(FontId::monospace(12.0)))
        .default_open(false)
        .show(ui, |ui| {
            details_panel(ui, poll);
        });
}

fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(FontId::monospace(11.0))
}

fn device_row(ui: &mut Ui, settings: &mut FrontendSettings, poll: &PollResult) {
    ui.horizontal(|ui| {
        ui.label(mono("DEVICE"));
        if poll.frame.connected.is_empty() {
            ui.label(mono("(none connected)"));
        } else {
            let mut preferred = settings.input.preferred_controller_profile_id.clone();
            let selected_name = preferred
                .as_ref()
                .and_then(|id| {
                    poll.frame
                        .connected
                        .iter()
                        .find(|p| &p.controller_profile_id == id)
                })
                .map(|p| p.name.as_str())
                .or_else(|| {
                    poll.frame.runtime_active.and_then(|id| {
                        poll.frame
                            .connected
                            .iter()
                            .find(|p| p.gamepad_id == id)
                            .map(|p| p.name.as_str())
                    })
                })
                .or_else(|| poll.frame.connected.first().map(|p| p.name.as_str()))
                .unwrap_or("(none)");

            let combo = egui::ComboBox::from_id_salt("input_device")
                .selected_text(RichText::new(selected_name).font(FontId::monospace(11.0)))
                .width(220.0)
                .show_ui(ui, |ui| {
                    for pad in &poll.frame.connected {
                        let selected = preferred.as_deref() == Some(&pad.controller_profile_id);
                        if ui
                            .selectable_label(
                                selected,
                                RichText::new(&pad.name).font(FontId::monospace(11.0)),
                            )
                            .clicked()
                        {
                            preferred = Some(pad.controller_profile_id.clone());
                        }
                    }
                });
            if combo.response.has_focus() {
                combo.response.surrender_focus();
            }
            if preferred != settings.input.preferred_controller_profile_id {
                settings.input.preferred_controller_profile_id = preferred;
                settings.save();
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label(mono(""));
        if poll.frame.unavailable {
            ui.label(mono("○ unavailable"));
        } else if poll.frame.runtime_active.is_some() {
            ui.colored_label(
                Color32::from_rgb(80, 200, 120),
                RichText::new("● CONNECTED").font(FontId::monospace(11.0)),
            );
        } else {
            ui.label(mono("○ disconnected"));
        }
    });

    ui.horizontal(|ui| {
        ui.label(mono(""));
        if let Some(label) = &poll.frame.last_input_label {
            ui.label(mono(format!("Last input: {label}")));
        } else {
            ui.label(mono("Last input: —"));
        }
    });
}

fn gilrs_status_panel(ui: &mut Ui, poll: &PollResult) {
    let diag = &poll.frame.diagnostics;
    ui.group(|ui| {
        ui.label(
            RichText::new("GILRS STATUS")
                .font(FontId::monospace(12.0))
                .strong(),
        );
        if diag.init_ok {
            ui.label(mono("init: OK"));
        } else {
            ui.colored_label(
                Color32::LIGHT_RED,
                mono(format!(
                    "init: ERROR — {}",
                    diag.init_error.as_deref().unwrap_or("unknown")
                )),
            );
        }
        ui.label(mono(format!(
            "connected gamepads: {}",
            poll.frame.connected.len()
        )));
        ui.label(mono(format!("active: {:?}", poll.frame.runtime_active)));
        ui.label(mono(diag.effective_hint.clone()));
        ui.label(mono(format!(
            "effective GB mask: 0x{:02X}",
            poll.effective_mask
        )));

        for pad in &poll.frame.connected {
            let active = poll.frame.runtime_active == Some(pad.gamepad_id);
            ui.separator();
            ui.label(mono(format!(
                "{} {}",
                if active { "▶" } else { " " },
                pad.name
            )));
            ui.label(mono(format!("session: {:?}", pad.gamepad_id)));
            ui.label(mono(format!("profile: {}", pad.controller_profile_id)));
            ui.label(mono(format!("uuid:    {}", pad.uuid_hex)));
            ui.label(mono(format!(
                "vid/pid: {:04x?}/{:04x?}",
                pad.vendor_id, pad.product_id
            )));
            ui.label(mono(format!("mapping: {}", pad.mapping)));
        }

        ui.separator();
        ui.label(
            RichText::new("LIVE EVENTS")
                .font(FontId::monospace(12.0))
                .strong(),
        );
        if let Some(last) = &diag.last_event {
            ui.label(mono(format!("last: {last}")));
        } else {
            ui.label(mono("last: (none yet — press a DualSense button to wake)"));
        }
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                if diag.event_log.is_empty() {
                    ui.label(mono("(no events)"));
                } else {
                    for line in diag.event_log.iter().rev() {
                        ui.label(mono(line.clone()));
                    }
                }
            });
    });
}

fn binding_table(
    ui: &mut Ui,
    settings: &FrontendSettings,
    input: &mut InputFrontend,
    poll: &PollResult,
) {
    let profile_id = poll
        .frame
        .active_profile_id
        .as_deref()
        .or(settings.input.preferred_controller_profile_id.as_deref());

    egui::Grid::new("input_bindings")
        .num_columns(3)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new("GAME BOY")
                    .font(FontId::monospace(11.0))
                    .strong(),
            );
            ui.label(
                RichText::new("KEYBOARD")
                    .font(FontId::monospace(11.0))
                    .strong(),
            );
            ui.label(
                RichText::new("GAMEPAD")
                    .font(FontId::monospace(11.0))
                    .strong(),
            );
            ui.end_row();

            for button in GameBoyButton::ALL {
                ui.label(mono(gb_button_label(button)));

                let key_label = keycode_label(settings.input.keyboard.key(button));
                if ui
                    .button(RichText::new(key_label).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::Keyboard(button));
                }

                let pad_label = active_pad_label(settings, button, profile_id);
                if ui
                    .button(RichText::new(pad_label).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::Gamepad(button));
                }

                ui.end_row();
            }

            // GBA shoulders (ignored on Game Boy carts).
            ui.label(mono("L"));
            {
                let code = keycode_from_name(&settings.input.keyboard.shoulder_l)
                    .unwrap_or(winit::keyboard::KeyCode::KeyA);
                if ui
                    .button(RichText::new(keycode_label(code)).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::KeyboardShoulderL);
                }
            }
            {
                let pad = active_shoulder_l(settings, profile_id);
                if ui
                    .button(RichText::new(pad_button_label(pad)).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::GamepadShoulderL);
                }
            }
            ui.end_row();

            ui.label(mono("R"));
            {
                let code = keycode_from_name(&settings.input.keyboard.shoulder_r)
                    .unwrap_or(winit::keyboard::KeyCode::KeyS);
                if ui
                    .button(RichText::new(keycode_label(code)).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::KeyboardShoulderR);
                }
            }
            {
                let pad = active_shoulder_r(settings, profile_id);
                if ui
                    .button(RichText::new(pad_button_label(pad)).font(FontId::monospace(11.0)))
                    .clicked()
                {
                    input.begin_listen(ListenTarget::GamepadShoulderR);
                }
            }
            ui.end_row();
        });
}

fn active_pad_label(
    settings: &FrontendSettings,
    button: GameBoyButton,
    profile_id: Option<&str>,
) -> String {
    let pad = if let Some(id) = profile_id
        && let Some(profile) = settings
            .input
            .gamepads
            .iter()
            .find(|p| p.controller_profile_id == id)
    {
        profile.map.button(button)
    } else {
        GamepadMap::default().button(button)
    };
    pad_button_label(pad).to_string()
}

fn active_shoulder_l(
    settings: &FrontendSettings,
    profile_id: Option<&str>,
) -> super::super::host_input::PadButton {
    if let Some(id) = profile_id
        && let Some(profile) = settings
            .input
            .gamepads
            .iter()
            .find(|p| p.controller_profile_id == id)
    {
        return profile.map.shoulder_l;
    }
    GamepadMap::default().shoulder_l
}

fn active_shoulder_r(
    settings: &FrontendSettings,
    profile_id: Option<&str>,
) -> super::super::host_input::PadButton {
    if let Some(id) = profile_id
        && let Some(profile) = settings
            .input
            .gamepads
            .iter()
            .find(|p| p.controller_profile_id == id)
    {
        return profile.map.shoulder_r;
    }
    GamepadMap::default().shoulder_r
}

fn deadzone_row(ui: &mut Ui, settings: &mut FrontendSettings) {
    ui.horizontal(|ui| {
        ui.label(mono("Deadzone"));
        let mut deadzone = settings.input.stick_deadzone;
        let response = ui.add(
            egui::Slider::new(&mut deadzone, 0.05..=0.95)
                .show_value(true)
                .fixed_decimals(2)
                .text("press threshold"),
        );
        if response.changed() {
            settings.input.stick_deadzone = deadzone;
            settings.input.clamp_deadzone();
            settings.save();
        }
        if response.has_focus() {
            response.surrender_focus();
        }
    });
}

fn live_tester(ui: &mut Ui, mask: u8) {
    ui.vertical_centered(|ui| {
        tester_button(ui, mask, GameBoyButton::Up);
        ui.horizontal(|ui| {
            tester_button(ui, mask, GameBoyButton::Left);
            tester_button(ui, mask, GameBoyButton::Down);
            tester_button(ui, mask, GameBoyButton::Right);
        });
        ui.horizontal(|ui| {
            tester_button(ui, mask, GameBoyButton::Select);
            tester_button(ui, mask, GameBoyButton::Start);
        });
        ui.horizontal(|ui| {
            tester_button(ui, mask, GameBoyButton::B);
            tester_button(ui, mask, GameBoyButton::A);
        });
    });
}

fn tester_button(ui: &mut Ui, mask: u8, button: GameBoyButton) {
    let pressed = mask & button_bit(button) != 0;
    let label = if pressed {
        RichText::new(format!("[{}]", gb_button_label(button)))
            .font(FontId::monospace(11.0))
            .strong()
            .color(Color32::WHITE)
    } else {
        RichText::new(gb_button_label(button))
            .font(FontId::monospace(11.0))
            .color(Color32::GRAY)
    };
    let fill = if pressed {
        Color32::from_rgb(60, 120, 200)
    } else {
        Color32::from_rgb(40, 40, 40)
    };
    ui.add(
        egui::Button::new(label)
            .fill(fill)
            .min_size(egui::vec2(56.0, 28.0)),
    );
}

fn details_panel(ui: &mut Ui, poll: &PollResult) {
    ui.label(
        RichText::new("RAW PAD")
            .font(FontId::monospace(12.0))
            .strong(),
    );
    ui.label(mono(format!(
        "LEFT STICK  ({:+.2}, {:+.2})",
        poll.frame.stick_x, poll.frame.stick_y
    )));
    let button = poll.frame.last_input_label.as_deref().unwrap_or("—");
    ui.label(mono(format!("BUTTON      {button}")));
    ui.add_space(4.0);
    ui.label(
        RichText::new("GAME BOY")
            .font(FontId::monospace(12.0))
            .strong(),
    );
    let mut any = false;
    for button in GameBoyButton::ALL {
        if poll.effective_mask & button_bit(button) != 0 {
            ui.label(mono(format!("◀ {}", gb_button_label(button))));
            any = true;
        }
    }
    if !any {
        ui.label(mono("(none)"));
    }
}

fn gb_button_label(button: GameBoyButton) -> &'static str {
    match button {
        GameBoyButton::Right => "Right",
        GameBoyButton::Left => "Left",
        GameBoyButton::Up => "Up",
        GameBoyButton::Down => "Down",
        GameBoyButton::A => "A",
        GameBoyButton::B => "B",
        GameBoyButton::Select => "Select",
        GameBoyButton::Start => "Start",
    }
}
