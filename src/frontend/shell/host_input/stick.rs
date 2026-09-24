//! Analog stick axis → digital D-pad bits (hysteresis).

use crate::frontend::shell::host_input::combine::button_bit;
use graycart::GameBoyButton;

#[derive(Debug, Default)]
pub struct StickDigital {
    right: bool,
    left: bool,
    up: bool,
    down: bool,
}

fn axis_positive(held: &mut bool, value: f32, press: f32) -> bool {
    let release = (press - 0.05).max(0.05);
    if *held {
        if value < release {
            *held = false;
        }
    } else if value >= press {
        *held = true;
    }
    *held
}

fn axis_negative(held: &mut bool, value: f32, press: f32) -> bool {
    let release = (press - 0.05).max(0.05);
    if *held {
        if value > -release {
            *held = false;
        }
    } else if value <= -press {
        *held = true;
    }
    *held
}

impl StickDigital {
    /// Positive X = Right; positive Y = Up (screen-space; gilrs Y is inverted here).
    pub fn update(&mut self, x: f32, y: f32, press: f32) -> u8 {
        let mut bits = 0u8;
        if axis_positive(&mut self.right, x, press) {
            bits |= button_bit(GameBoyButton::Right);
        }
        if axis_negative(&mut self.left, x, press) {
            bits |= button_bit(GameBoyButton::Left);
        }
        // Flip gilrs/SDL Y so stick-down → GB Down.
        if axis_positive(&mut self.up, y, press) {
            bits |= button_bit(GameBoyButton::Up);
        }
        if axis_negative(&mut self.down, y, press) {
            bits |= button_bit(GameBoyButton::Down);
        }
        bits
    }
}

#[cfg(test)]
mod tests;
