//! Shared Machine Monitor widgets.

use super::super::history::Sparkline;
use egui::{CollapsingHeader, Color32, FontId, RichText, Sense, Stroke, Ui, Vec2};

pub(super) fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(8))
        .corner_radius(2.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .font(FontId::monospace(10.0))
                    .color(Color32::from_rgb(140, 180, 160))
                    .strong(),
            );
            ui.add_space(2.0);
            add(ui);
        });
}

pub(super) fn collapsing(ui: &mut Ui, title: &str, open: &mut bool, add: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .corner_radius(2.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let resp = CollapsingHeader::new(
                RichText::new(title)
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(140, 180, 160))
                    .strong(),
            )
            .default_open(*open)
            .show(ui, |ui| {
                add(ui);
            });
            *open = resp.openness > 0.5;
        });
}

pub(super) fn mono_kv(ui: &mut Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [56.0, 12.0],
            egui::Label::new(
                RichText::new(k)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.label(RichText::new(v).font(FontId::monospace(11.0)));
    });
}

pub(super) fn mono_inline(ui: &mut Ui, k: &str, v: &str) {
    ui.label(
        RichText::new(format!("{k} "))
            .font(FontId::monospace(10.0))
            .color(Color32::GRAY),
    );
    ui.label(RichText::new(v).font(FontId::monospace(11.0)));
    ui.add_space(8.0);
}

pub(super) fn b(v: bool) -> char {
    if v { '1' } else { '0' }
}

pub(super) fn channel(ui: &mut Ui, name: &str, ch: &graycart::ChannelDebug) {
    let color = if ch.active {
        Color32::from_rgb(220, 180, 60)
    } else {
        Color32::from_gray(70)
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(name)
                .font(FontId::monospace(10.0))
                .color(if ch.active {
                    Color32::LIGHT_GRAY
                } else {
                    Color32::DARK_GRAY
                }),
        );
        bar(ui, ch.volume as f32 / 15.0, color, 48.0);
        ui.label(
            RichText::new(if ch.active { "ON" } else { "off" })
                .font(FontId::monospace(10.0))
                .color(if ch.active {
                    Color32::from_rgb(80, 200, 120)
                } else {
                    Color32::DARK_GRAY
                }),
        );
        ui.label(
            RichText::new(format!("v{} f{}", ch.volume, ch.frequency))
                .font(FontId::monospace(10.0))
                .color(Color32::GRAY),
        );
    });
}

pub(super) fn bar(ui: &mut Ui, fill: f32, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(35));
    let mut fill_r = rect;
    fill_r.set_width(rect.width() * fill.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill_r, 1.0, color);
}

pub(super) fn sparkline(ui: &mut Ui, spark: &Sparkline, hint_max: f32, color: Color32) {
    let samples = spark.samples();
    // Fixed geometry: height is constant; width is the parent's available width.
    const HEIGHT: f32 = 22.0;
    let width = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(20));
    if samples.len() < 2 {
        return;
    }
    let max_v = samples
        .iter()
        .cloned()
        .fold(hint_max.max(0.001), f32::max)
        .max(0.001);
    let n = (samples.len() - 1) as f32;
    let points: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                rect.left() + (i as f32 / n) * rect.width(),
                rect.bottom() - (v / max_v).clamp(0.0, 1.0) * rect.height(),
            )
        })
        .collect();
    for w in points.windows(2) {
        ui.painter()
            .line_segment([w[0], w[1]], Stroke::new(1.1_f32, color));
    }
}

pub(super) fn reg_row(ui: &mut Ui, a_name: &str, a_val: u16, b_name: &str, b_val: u16) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_sized(
            [28.0, 14.0],
            egui::Label::new(
                RichText::new(a_name)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [40.0, 14.0],
            egui::Label::new(RichText::new(format!("{a_val:04X}")).font(FontId::monospace(11.0))),
        );
        ui.add_space(16.0);
        ui.add_sized(
            [28.0, 14.0],
            egui::Label::new(
                RichText::new(b_name)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [40.0, 14.0],
            egui::Label::new(RichText::new(format!("{b_val:04X}")).font(FontId::monospace(11.0))),
        );
    });
}
