//! Keyboard + gamepad masks → effective mask; press/release edges.

use graycart::GameBoyButton;

#[derive(Debug, Default)]
pub struct EdgeTracker {
    prev: u8,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct InputEdges {
    pub pressed: Vec<GameBoyButton>,
    pub released: Vec<GameBoyButton>,
}

pub fn combine(keyboard: u8, gamepad: u8) -> u8 {
    keyboard | gamepad
}

pub fn button_bit(b: GameBoyButton) -> u8 {
    1 << GameBoyButton::ALL.iter().position(|&x| x == b).unwrap()
}

impl EdgeTracker {
    pub fn reset(&mut self) {
        self.prev = 0;
    }

    pub fn edges(&mut self, now: u8) -> InputEdges {
        let mut out = InputEdges::default();
        for (i, button) in GameBoyButton::ALL.iter().enumerate() {
            let bit = 1u8 << i;
            let down = now & bit != 0;
            let was = self.prev & bit != 0;
            if down && !was {
                out.pressed.push(*button);
            } else if !down && was {
                out.released.push(*button);
            }
        }
        self.prev = now;
        out
    }
}

#[cfg(test)]
mod tests;
