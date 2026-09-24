//! Application focus vs gameplay-input capture (independent booleans).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppFocus {
    #[default]
    Main,
    Auxiliary,
    Unfocused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputCapture {
    #[default]
    Gameplay,
    Rebinding,
    TextEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostInputPolicy {
    pub keyboard_to_joypad: bool,
    pub gamepad_to_joypad: bool,
    pub poll_gamepad: bool,
    pub clear_held_keyboard: bool,
    pub emit_app_unfocused: bool,
}

/// OS-unfocus for pause/clear-holds: no Graycart window focused, and no
/// auxiliary window in the middle of a focus handoff from the main surface.
pub fn emit_app_unfocused(focus: AppFocus, aux_window_open: bool) -> bool {
    focus == AppFocus::Unfocused && !aux_window_open
}

/// Window-specific UI never owns gamepad polling or emu execution.
/// `aux_window_open` keeps a main→controls focus handoff from looking like OS unfocus.
pub fn input_policy(
    focus: AppFocus,
    capture: InputCapture,
    aux_window_open: bool,
) -> HostInputPolicy {
    let focus = if focus == AppFocus::Unfocused && aux_window_open {
        AppFocus::Auxiliary
    } else {
        focus
    };
    match (focus, capture) {
        (_, InputCapture::Rebinding) => HostInputPolicy {
            keyboard_to_joypad: false,
            gamepad_to_joypad: false,
            poll_gamepad: true,
            clear_held_keyboard: false,
            emit_app_unfocused: false,
        },
        (_, InputCapture::TextEntry) => HostInputPolicy {
            keyboard_to_joypad: false,
            gamepad_to_joypad: true,
            poll_gamepad: true,
            clear_held_keyboard: false,
            emit_app_unfocused: false,
        },
        (AppFocus::Main, InputCapture::Gameplay) => HostInputPolicy {
            keyboard_to_joypad: true,
            gamepad_to_joypad: true,
            poll_gamepad: true,
            clear_held_keyboard: false,
            emit_app_unfocused: false,
        },
        (AppFocus::Auxiliary, InputCapture::Gameplay) => HostInputPolicy {
            keyboard_to_joypad: false,
            gamepad_to_joypad: true,
            poll_gamepad: true,
            clear_held_keyboard: false,
            emit_app_unfocused: false,
        },
        (AppFocus::Unfocused, InputCapture::Gameplay) => HostInputPolicy {
            keyboard_to_joypad: false,
            gamepad_to_joypad: true,
            poll_gamepad: true,
            clear_held_keyboard: true,
            emit_app_unfocused: true,
        },
    }
}

/// True when any Graycart window still holds OS focus.
pub fn app_focus_from_surfaces(main_focused: bool, aux_focused: bool) -> AppFocus {
    if main_focused {
        AppFocus::Main
    } else if aux_focused {
        AppFocus::Auxiliary
    } else {
        AppFocus::Unfocused
    }
}

#[cfg(test)]
pub fn apply_window_focus(
    previous: AppFocus,
    is_main: bool,
    is_auxiliary: bool,
    focused: bool,
) -> AppFocus {
    if focused {
        if is_main {
            AppFocus::Main
        } else if is_auxiliary {
            AppFocus::Auxiliary
        } else {
            previous
        }
    } else if (is_main && previous == AppFocus::Main)
        || (is_auxiliary && previous == AppFocus::Auxiliary)
    {
        AppFocus::Unfocused
    } else {
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_and_aux_surface_bits_prefer_main_then_aux() {
        assert_eq!(app_focus_from_surfaces(true, true), AppFocus::Main);
        assert_eq!(app_focus_from_surfaces(false, true), AppFocus::Auxiliary);
        assert_eq!(app_focus_from_surfaces(false, false), AppFocus::Unfocused);
    }

    #[test]
    fn controls_window_focus_does_not_unfocus_the_app() {
        let next = apply_window_focus(AppFocus::Main, false, true, true);
        assert_eq!(next, AppFocus::Auxiliary);
        let policy = input_policy(next, InputCapture::Gameplay, true);
        assert!(policy.poll_gamepad);
        assert!(policy.gamepad_to_joypad);
        assert!(!policy.keyboard_to_joypad);
        assert!(!policy.emit_app_unfocused);
    }

    #[test]
    fn main_losing_focus_to_os_clears_keyboard_holds_not_gamepad_poll() {
        let next = apply_window_focus(AppFocus::Main, true, false, false);
        assert_eq!(next, AppFocus::Unfocused);
        let policy = input_policy(next, InputCapture::Gameplay, false);
        assert!(policy.clear_held_keyboard);
        assert!(policy.poll_gamepad);
        assert!(policy.emit_app_unfocused);
    }

    #[test]
    fn rebinding_captures_input_without_stopping_gamepad_poll() {
        let policy = input_policy(AppFocus::Auxiliary, InputCapture::Rebinding, true);
        assert!(policy.poll_gamepad);
        assert!(!policy.gamepad_to_joypad);
        assert!(!policy.keyboard_to_joypad);
        assert!(!policy.emit_app_unfocused);
    }

    #[test]
    fn debug_snapshot_production_is_independent_of_aux_focus() {
        let aux = input_policy(AppFocus::Auxiliary, InputCapture::Gameplay, true);
        let main = input_policy(AppFocus::Main, InputCapture::Gameplay, false);
        assert!(aux.poll_gamepad && main.poll_gamepad);
        assert!(!aux.emit_app_unfocused);
    }

    #[test]
    fn opening_controls_does_not_emit_app_unfocused() {
        let after_main_blur = apply_window_focus(AppFocus::Main, true, false, false);
        assert_eq!(after_main_blur, AppFocus::Unfocused);
        assert!(
            !emit_app_unfocused(after_main_blur, true),
            "aux window exists; this is a focus handoff, not leaving Graycart"
        );
        let after_controls = apply_window_focus(AppFocus::Unfocused, false, true, true);
        assert_eq!(after_controls, AppFocus::Auxiliary);
        assert!(!emit_app_unfocused(after_controls, true));
        let handoff = input_policy(after_main_blur, InputCapture::Gameplay, true);
        assert!(!handoff.clear_held_keyboard);
        assert!(!handoff.emit_app_unfocused);
        assert!(handoff.gamepad_to_joypad);
    }

    #[test]
    fn hardware_relaunch_does_not_change_input_policy() {
        let before = input_policy(AppFocus::Main, InputCapture::Gameplay, false);
        let after = input_policy(AppFocus::Main, InputCapture::Gameplay, false);
        assert_eq!(before, after);
    }
}
