//! Pure host keyboard → GBA keypad bit map (no winit / egui / cpal / gilrs).
//!
//! Default letters match the Game Boy shell for shared buttons, plus A/S for L/R.
//! Key names use the same Debug strings as `winit::keyboard::KeyCode`.

/// Host key name → GBA KEYINPUT bit (GBATEK: 0=A … 9=L).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyMap {
    /// Parallel to bits 0..=9. Empty string = unbound.
    keys: [String; 10],
}

impl HostKeyMap {
    /// True when some host key in this map is bound to the given KEYINPUT bit.
    pub fn maps_to_gba_bit(&self, bit: u16) -> bool {
        let Some(slot) = self.keys.get(bit as usize) else {
            return false;
        };
        !slot.is_empty()
    }

    pub fn key_for_bit(&self, bit: u16) -> Option<&str> {
        self.keys
            .get(bit as usize)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn pressed_mask(&self, down: &[&str]) -> u16 {
        let mut mask = 0u16;
        for (bit, name) in self.keys.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            if down.contains(&name.as_str()) {
                mask |= 1 << bit;
            }
        }
        mask
    }
}

/// Default GBA host keys (Game Boy letters for shared buttons, plus shoulders).
///
/// Bits: 0=A(X), 1=B(Z), 2=Select(ShiftRight), 3=Start(Enter),
/// 4=Right, 5=Left, 6=Up, 7=Down, 8=R(S), 9=L(A).
pub fn default_host_keys_for_test() -> HostKeyMap {
    HostKeyMap {
        keys: [
            "KeyX".into(),       // A
            "KeyZ".into(),       // B
            "ShiftRight".into(), // Select
            "Enter".into(),      // Start
            "ArrowRight".into(), // Right
            "ArrowLeft".into(),  // Left
            "ArrowUp".into(),    // Up
            "ArrowDown".into(),  // Down
            "KeyS".into(),       // R
            "KeyA".into(),       // L
        ],
    }
}

/// Same as [`default_host_keys_for_test`]; production seed for the configurator.
pub fn default_gba_host_keys() -> HostKeyMap {
    default_host_keys_for_test()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gba_bindings_match_the_game_boy_letters_plus_shoulders() {
        let keys = default_host_keys_for_test();
        assert!(keys.maps_to_gba_bit(0)); // X -> A
        assert!(keys.maps_to_gba_bit(1)); // Z -> B
        assert!(keys.maps_to_gba_bit(9)); // A -> L
        assert!(keys.maps_to_gba_bit(8)); // S -> R
    }

    #[test]
    fn shoulder_keys_set_keyinput_bits_9_and_8() {
        let keys = default_host_keys_for_test();
        let l_only = keys.pressed_mask(&["KeyA"]);
        assert_eq!(l_only & (1 << 9), 1 << 9, "KeyA must set L (bit 9)");
        assert_eq!(l_only & (1 << 8), 0, "KeyA must not set R");

        let r_only = keys.pressed_mask(&["KeyS"]);
        assert_eq!(r_only & (1 << 8), 1 << 8, "KeyS must set R (bit 8)");
        assert_eq!(r_only & (1 << 9), 0, "KeyS must not set L");

        let both = keys.pressed_mask(&["KeyA", "KeyS"]);
        assert_eq!(both & (1 << 9), 1 << 9);
        assert_eq!(both & (1 << 8), 1 << 8);
    }
}
