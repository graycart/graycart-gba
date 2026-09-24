//! Save presented RGBA (CPU present + host effects) as PNG under `~/Pictures/Graycart/`.

use png::{BitDepth, ColorType, Encoder};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub fn pictures_dir() -> PathBuf {
    let base = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Graycart");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn screenshot_filename(title: &str) -> String {
    let safe = graycart::sanitize_title(title);
    let now = chrono_lite_timestamp();
    format!("{safe}_{now}.png")
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamp_utc(secs)
}

fn format_timestamp_utc(secs: u64) -> String {
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = secs / SECONDS_PER_DAY;
    let rem = secs % SECONDS_PER_DAY;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    let mut z = days + 719_468;
    let era = z / 146_097;
    z -= era * 146_097;
    let yoe = (z - z / 1460 + z / 36_524 - z / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = z - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        year, m, d, hour, min, sec
    )
}

pub fn write_presented_png(
    title: &str,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> std::io::Result<PathBuf> {
    write_presented_png_to(&pictures_dir(), title, rgba, width, height)
}

pub fn write_presented_png_to(
    base: &Path,
    title: &str,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(base)?;
    let path = base.join(screenshot_filename(title));
    let file = fs::File::create(&path)?;
    let mut enc = Encoder::new(BufWriter::new(file), width, height);
    enc.set_color(ColorType::Rgba);
    enc.set_depth(BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(path)
}

#[cfg(test)]
mod tests;
