//! Game viewport layout — scale a framebuffer into the content rect below the menu bar.

use super::DisplayMode;

/// Compute the on-screen size for a `src_w`×`src_h` framebuffer inside `available`.
pub fn game_image_size(
    available_w: f32,
    available_h: f32,
    integer_scaling: bool,
    mode: DisplayMode,
    src_w: f32,
    src_h: f32,
) -> (f32, f32) {
    let tw = src_w;
    let th = src_h;
    if available_w <= 0.0 || available_h <= 0.0 || tw <= 0.0 || th <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = if integer_scaling && !mode.prefers_linear_filter() {
        (available_w / tw).min(available_h / th).floor().max(1.0)
    } else {
        (available_w / tw).min(available_h / th).max(0.0)
    };
    (tw * scale, th * scale)
}

#[cfg(test)]
mod tests;
