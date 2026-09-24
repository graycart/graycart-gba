//! History sparklines card.

use super::DebugFrame;
use super::format::{fmt_audio_queue, fmt_overview_ms};
use super::widgets::{card, sparkline};
use egui::{Color32, FontId, RichText, Ui};

pub(super) fn draw_graphs(ui: &mut Ui, frame: &DebugFrame<'_>) {
    let host = frame.host;
    card(ui, "GRAPHS", |ui| {
        graph_caption(ui, "REALTIME", "");
        sparkline(
            ui,
            &frame.history.emu_mhz,
            4.2,
            Color32::from_rgb(120, 220, 140),
        );
        graph_caption(ui, "FRAME", &fmt_overview_ms(host.frame_ms_avg));
        sparkline(
            ui,
            &frame.history.frame_ms,
            20.0,
            Color32::from_rgb(200, 160, 80),
        );
        graph_caption(
            ui,
            "AUDIO Q",
            &fmt_audio_queue(host.audio_queued, host.audio_target),
        );
        sparkline(
            ui,
            &frame.history.audio_queue,
            host.audio_target.max(1) as f32,
            Color32::from_rgb(100, 160, 255),
        );
    });
}

fn graph_caption(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [72.0, 12.0],
            egui::Label::new(
                RichText::new(label)
                    .font(FontId::monospace(10.0))
                    .color(Color32::GRAY),
            ),
        );
        ui.add_sized(
            [120.0, 12.0],
            egui::Label::new(
                RichText::new(value)
                    .font(FontId::monospace(10.0))
                    .color(Color32::DARK_GRAY),
            )
            .halign(egui::Align::LEFT),
        );
    });
}
