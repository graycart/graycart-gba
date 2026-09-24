//! Gilrs hotplug, controller profile selection, and active-pad sampling.

use super::{InputSettings, PadButton, StickDigital, button_bit};
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs, MappingSource};
use graycart::GameBoyButton;
use std::collections::{HashMap, VecDeque};
use std::sync::Once;

static LOG_GILRS_FAILURE: Once = Once::new();
const EVENT_LOG_CAP: usize = 24;

#[derive(Debug, Clone)]
pub struct ConnectedPad {
    pub gamepad_id: GamepadId,
    pub controller_profile_id: String,
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub mapping: String,
    pub uuid_hex: String,
}

#[derive(Debug, Clone, Default)]
pub struct GamepadDiagnostics {
    pub init_ok: bool,
    pub init_error: Option<String>,
    pub event_log: Vec<String>,
    pub last_event: Option<String>,
    pub effective_hint: String,
}

#[derive(Debug, Clone, Default)]
pub struct GamepadFrame {
    pub connected: Vec<ConnectedPad>,
    pub runtime_active: Option<GamepadId>,
    pub active_profile_id: Option<String>,
    pub settings_dirty: bool,
    pub pressed_buttons: Vec<PadButton>,
    pub pad_mask: u8,
    /// GBA L/R bits (9 and 8) from the active pad; ignored for Game Boy carts.
    pub shoulder_mask: u16,
    pub stick_x: f32,
    pub stick_y: f32,
    pub last_input_label: Option<String>,
    pub unavailable: bool,
    pub diagnostics: GamepadDiagnostics,
}

pub struct GamepadHub {
    gilrs: Option<Gilrs>,
    init_error: Option<String>,
    pads: HashMap<GamepadId, ConnectedPad>,
    connect_order: VecDeque<GamepadId>,
    runtime_active: Option<GamepadId>,
    stick: StickDigital,
    stick_bits: u8,
    last_input_label: Option<String>,
    event_log: VecDeque<String>,
    last_event: Option<String>,
    bootstrapped: bool,
}

impl Default for GamepadHub {
    fn default() -> Self {
        Self::new()
    }
}

impl GamepadHub {
    pub fn new() -> Self {
        let (gilrs, init_error) = match Gilrs::new() {
            Ok(gilrs) => (Some(gilrs), None),
            Err(error) => {
                LOG_GILRS_FAILURE.call_once(|| {
                    eprintln!("gamepad support unavailable: {error}");
                });
                (None, Some(error.to_string()))
            }
        };
        Self {
            gilrs,
            init_error,
            pads: HashMap::new(),
            connect_order: VecDeque::new(),
            runtime_active: None,
            stick: StickDigital::default(),
            stick_bits: 0,
            last_input_label: None,
            event_log: VecDeque::with_capacity(EVENT_LOG_CAP),
            last_event: None,
            bootstrapped: false,
        }
    }

    pub fn pump(&mut self, settings: &mut InputSettings) -> GamepadFrame {
        let Some(mut gilrs) = self.gilrs.take() else {
            return self.frame_unavailable();
        };

        let mut pressed_buttons = Vec::new();
        let mut settings_dirty = false;
        let debug = std::env::var_os("GRAYCART_INPUT_DEBUG").is_some();

        // Drain events first — on macOS a DualSense already plugged in may not
        // appear in gamepads() until Connected is observed via next_event().
        let mut events = Vec::new();
        while let Some(event) = gilrs.next_event() {
            events.push(event);
        }

        for event in events {
            let line = format_event(&event.event, event.id);
            self.push_event(line.clone());
            if debug {
                eprintln!("input:gilrs {line}");
            }

            // Any traffic from an unknown id → register immediately (don't wait
            // solely for Connected / gamepads() lag).
            if !self.pads.contains_key(&event.id) && gilrs.gamepad(event.id).is_connected() {
                let pad = connected_pad(event.id, gilrs.gamepad(event.id));
                settings_dirty |=
                    register_pad(&mut self.pads, &mut self.connect_order, settings, pad);
            }

            match event.event {
                EventType::Connected => {
                    let pad = connected_pad(event.id, gilrs.gamepad(event.id));
                    settings_dirty |=
                        register_pad(&mut self.pads, &mut self.connect_order, settings, pad);
                    if self.runtime_active.is_none() {
                        self.runtime_active = Some(event.id);
                    }
                }
                EventType::Disconnected => {
                    self.pads.remove(&event.id);
                    self.connect_order.retain(|id| *id != event.id);
                    if self.runtime_active == Some(event.id) {
                        self.runtime_active = newest_remaining(&self.connect_order);
                    }
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(pad_btn) = pad_button_from_gilrs(button) {
                        if self.runtime_active == Some(event.id) {
                            pressed_buttons.push(pad_btn);
                        }
                        self.last_input_label =
                            Some(super::mapping::pad_button_label(pad_btn).into());
                    } else {
                        self.last_input_label = Some(format!("Raw {button:?}"));
                    }
                    // First press often wakes DualSense; ensure this pad is active
                    // if we have no active pad yet.
                    if self.runtime_active.is_none() {
                        self.runtime_active = Some(event.id);
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if pad_button_from_gilrs(button).is_none() {
                        self.last_input_label = Some(format!("Raw {button:?} up"));
                    }
                }
                EventType::AxisChanged(axis, value, _) if value.abs() > 0.35 => {
                    self.last_input_label = Some(format!("{axis:?} {value:+.2}"));
                    if self.runtime_active.is_none() {
                        self.runtime_active = Some(event.id);
                    }
                }
                _ => {}
            }
        }

        // Reconcile currently connected pads (startup + hotplug). Critical for
        // controllers already attached before Graycart launched — once gilrs
        // exposes them via gamepads(), we must register without waiting for a
        // future Connected we may have already missed.
        let present: Vec<_> = gilrs
            .gamepads()
            .map(|(id, gamepad)| connected_pad(id, gamepad))
            .collect();
        let present_ids: Vec<_> = present.iter().map(|pad| pad.gamepad_id).collect();
        self.pads.retain(|id, _| present_ids.contains(id));
        self.connect_order.retain(|id| present_ids.contains(id));
        for pad in present {
            if !self.pads.contains_key(&pad.gamepad_id) {
                settings_dirty |=
                    register_pad(&mut self.pads, &mut self.connect_order, settings, pad);
            }
        }
        if !self.bootstrapped {
            self.bootstrapped = true;
            if debug {
                eprintln!("input:gilrs bootstrap connected={}", self.pads.len());
            }
        }

        let previous_active = self.runtime_active;
        if let Some(preferred) = settings.preferred_controller_profile_id.as_deref()
            && let Some(id) = self.newest_pad_for_profile(preferred)
        {
            self.runtime_active = Some(id);
        } else if self
            .runtime_active
            .is_none_or(|id| !self.pads.contains_key(&id))
        {
            self.runtime_active = self.connect_order.back().copied();
        }
        if self.runtime_active != previous_active {
            self.stick = StickDigital::default();
            self.stick_bits = 0;
        }

        let mut pad_mask = 0;
        let mut shoulder_mask = 0u16;
        let mut stick_x = 0.0;
        let mut stick_y = 0.0;
        let active_profile_id = self
            .runtime_active
            .and_then(|id| self.pads.get(&id))
            .map(|pad| pad.controller_profile_id.clone());
        if let (Some(id), Some(profile_id)) = (self.runtime_active, active_profile_id.as_deref()) {
            let gamepad = gilrs.gamepad(id);
            let map = settings
                .gamepads
                .iter()
                .find(|profile| profile.controller_profile_id == profile_id)
                .map(|profile| profile.map.clone())
                .unwrap_or_default();
            for gb in GameBoyButton::ALL {
                if gamepad.is_pressed(map.button(gb).into()) {
                    pad_mask |= button_bit(gb);
                }
            }
            if gamepad.is_pressed(map.shoulder_l.into()) {
                shoulder_mask |= 1 << 9;
            }
            if gamepad.is_pressed(map.shoulder_r.into()) {
                shoulder_mask |= 1 << 8;
            }
            stick_x = gamepad.value(Axis::LeftStickX);
            stick_y = gamepad.value(Axis::LeftStickY);
            let new_stick_bits =
                self.stick
                    .update(stick_x, stick_y, settings.stick_deadzone.clamp(0.05, 0.95));
            if let Some(label) = stick_edge_label(self.stick_bits, new_stick_bits) {
                self.last_input_label = Some(label.into());
            }
            self.stick_bits = new_stick_bits;
            pad_mask |= new_stick_bits;
        } else {
            self.stick = StickDigital::default();
            self.stick_bits = 0;
        }

        let diagnostics = GamepadDiagnostics {
            init_ok: true,
            init_error: None,
            event_log: self.event_log.iter().cloned().collect(),
            last_event: self.last_event.clone(),
            effective_hint: format!("pad_mask=0x{pad_mask:02X} active={:?}", self.runtime_active),
        };

        let frame = GamepadFrame {
            connected: self.connected_in_order(),
            runtime_active: self.runtime_active,
            active_profile_id,
            settings_dirty,
            pressed_buttons,
            pad_mask,
            shoulder_mask,
            stick_x,
            stick_y,
            last_input_label: self.last_input_label.clone(),
            unavailable: false,
            diagnostics,
        };
        self.gilrs = Some(gilrs);
        frame
    }

    fn push_event(&mut self, line: String) {
        self.last_event = Some(line.clone());
        if self.event_log.len() >= EVENT_LOG_CAP {
            self.event_log.pop_front();
        }
        self.event_log.push_back(line);
    }

    fn frame_unavailable(&self) -> GamepadFrame {
        GamepadFrame {
            connected: Vec::new(),
            runtime_active: None,
            active_profile_id: None,
            settings_dirty: false,
            pressed_buttons: Vec::new(),
            pad_mask: 0,
            shoulder_mask: 0,
            stick_x: 0.0,
            stick_y: 0.0,
            last_input_label: None,
            unavailable: true,
            diagnostics: GamepadDiagnostics {
                init_ok: false,
                init_error: self.init_error.clone(),
                event_log: self.event_log.iter().cloned().collect(),
                last_event: self.last_event.clone(),
                effective_hint: "gilrs unavailable".into(),
            },
        }
    }

    fn newest_pad_for_profile(&self, profile: &str) -> Option<GamepadId> {
        self.connect_order.iter().rev().copied().find(|id| {
            self.pads
                .get(id)
                .is_some_and(|pad| pad.controller_profile_id == profile)
        })
    }

    fn connected_in_order(&self) -> Vec<ConnectedPad> {
        self.connect_order
            .iter()
            .filter_map(|id| self.pads.get(id).cloned())
            .collect()
    }
}

fn format_event(event: &EventType, id: GamepadId) -> String {
    match event {
        EventType::Connected => format!("Connected({id:?})"),
        EventType::Disconnected => format!("Disconnected({id:?})"),
        EventType::ButtonPressed(b, c) => format!("ButtonPressed({id:?}, {b:?}, {c:?})"),
        EventType::ButtonReleased(b, c) => format!("ButtonReleased({id:?}, {b:?}, {c:?})"),
        EventType::ButtonChanged(b, v, c) => {
            format!("ButtonChanged({id:?}, {b:?}, {v:.2}, {c:?})")
        }
        EventType::AxisChanged(a, v, c) => format!("AxisChanged({id:?}, {a:?}, {v:+.2}, {c:?})"),
        other => format!("{other:?} ({id:?})"),
    }
}

fn mapping_label(source: MappingSource) -> String {
    match source {
        MappingSource::SdlMappings => "SdlMappings".into(),
        MappingSource::Driver => "Driver".into(),
        MappingSource::None => "None".into(),
    }
}

fn connected_pad(id: GamepadId, gamepad: gilrs::Gamepad<'_>) -> ConnectedPad {
    let uuid = gamepad.uuid();
    ConnectedPad {
        gamepad_id: id,
        controller_profile_id: profile_key(
            uuid,
            gamepad.vendor_id(),
            gamepad.product_id(),
            gamepad.name(),
        ),
        name: gamepad.name().to_string(),
        vendor_id: gamepad.vendor_id(),
        product_id: gamepad.product_id(),
        mapping: mapping_label(gamepad.mapping_source()),
        uuid_hex: uuid.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

fn register_pad(
    pads: &mut HashMap<GamepadId, ConnectedPad>,
    connect_order: &mut VecDeque<GamepadId>,
    settings: &mut InputSettings,
    pad: ConnectedPad,
) -> bool {
    settings.upsert_profile(pad.controller_profile_id.clone(), pad.name.clone());
    let settings_dirty = initialize_preferred_profile(settings, pad.controller_profile_id.as_str());
    connect_order.retain(|id| *id != pad.gamepad_id);
    connect_order.push_back(pad.gamepad_id);
    pads.insert(pad.gamepad_id, pad);
    settings_dirty
}

fn initialize_preferred_profile(settings: &mut InputSettings, profile_id: &str) -> bool {
    if settings.preferred_controller_profile_id.is_some() {
        return false;
    }
    settings.preferred_controller_profile_id = Some(profile_id.to_owned());
    true
}

fn newest_remaining<T: Copy>(connect_order: &VecDeque<T>) -> Option<T> {
    connect_order.back().copied()
}

pub fn pad_button_from_gilrs(button: Button) -> Option<PadButton> {
    Some(match button {
        Button::South => PadButton::South,
        Button::East => PadButton::East,
        Button::West => PadButton::West,
        Button::North => PadButton::North,
        Button::DPadUp => PadButton::DPadUp,
        Button::DPadDown => PadButton::DPadDown,
        Button::DPadLeft => PadButton::DPadLeft,
        Button::DPadRight => PadButton::DPadRight,
        Button::Select => PadButton::Select,
        Button::Start => PadButton::Start,
        Button::LeftTrigger | Button::LeftTrigger2 => PadButton::LeftShoulder,
        Button::RightTrigger | Button::RightTrigger2 => PadButton::RightShoulder,
        _ => return None,
    })
}

impl From<PadButton> for Button {
    fn from(button: PadButton) -> Self {
        match button {
            PadButton::South => Self::South,
            PadButton::East => Self::East,
            PadButton::West => Self::West,
            PadButton::North => Self::North,
            PadButton::DPadUp => Self::DPadUp,
            PadButton::DPadDown => Self::DPadDown,
            PadButton::DPadLeft => Self::DPadLeft,
            PadButton::DPadRight => Self::DPadRight,
            PadButton::Select => Self::Select,
            PadButton::Start => Self::Start,
            PadButton::LeftShoulder => Self::LeftTrigger,
            PadButton::RightShoulder => Self::RightTrigger,
        }
    }
}

fn stick_edge_label(previous: u8, current: u8) -> Option<&'static str> {
    [
        (GameBoyButton::Right, "Left Stick Right"),
        (GameBoyButton::Left, "Left Stick Left"),
        (GameBoyButton::Up, "Left Stick Up"),
        (GameBoyButton::Down, "Left Stick Down"),
    ]
    .into_iter()
    .find(|(button, _)| current & button_bit(*button) != 0 && previous & button_bit(*button) == 0)
    .map(|(_, label)| label)
}

pub fn profile_key(
    uuid: [u8; 16],
    vendor: Option<u16>,
    product: Option<u16>,
    name: &str,
) -> String {
    if uuid != [0; 16] {
        return uuid.iter().map(|byte| format!("{byte:02x}")).collect();
    }

    format!(
        "{:04x}:{:04x}:{name}",
        vendor.unwrap_or(0),
        product.unwrap_or(0)
    )
}

#[cfg(test)]
mod tests;
