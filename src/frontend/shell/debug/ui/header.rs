//! Fixed header: status banner, title, capture toolbar, report viewer.

use super::super::host::HostMetrics;
use super::widgets::bar;
use super::{DebugFrame, MonitorAction};
use egui::{Color32, FontFamily, FontId, RichText, Ui};

#[cfg(not(debug_assertions))]
use super::format::fmt_overview_pct;
#[cfg(not(debug_assertions))]
use super::health::{audio_health, health_color};

pub(super) fn draw_status_banner(ui: &mut Ui, host: &HostMetrics) {
    #[cfg(debug_assertions)]
    {
        let _ = host;
        ui.colored_label(
            Color32::from_rgb(220, 160, 40),
            RichText::new("BUILD DEBUG  ·  not representative of play performance")
                .font(FontId::monospace(12.0))
                .strong(),
        );
    }
    #[cfg(not(debug_assertions))]
    {
        let rt = host.realtime_pct();
        let (mark, color) = if rt >= 99.0 {
            ("OK  ", Color32::from_rgb(80, 220, 120))
        } else if rt >= 97.0 {
            ("WARN", Color32::from_rgb(220, 180, 60))
        } else {
            ("BAD ", Color32::from_rgb(220, 80, 80))
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("BUILD RELEASE")
                    .font(FontId::monospace(12.0))
                    .color(Color32::from_rgb(160, 200, 180))
                    .strong(),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("REALTIME {}", fmt_overview_pct(rt)))
                    .font(FontId::monospace(12.0))
                    .color(color)
                    .strong(),
            );
            ui.label(
                RichText::new(mark)
                    .font(FontId::monospace(12.0))
                    .color(color),
            );
            let audio = audio_health(host);
            ui.separator();
            ui.label(
                RichText::new(format!("AUDIO {}", audio.label))
                    .font(FontId::monospace(12.0))
                    .color(health_color(audio.level)),
            );
        });
    }
}

pub(super) fn draw_title_row(ui: &mut Ui, frame: &DebugFrame<'_>) {
    let host = frame.host;
    let running = frame.machine.is_some() || frame.arm_lines.is_some();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Graycart // Machine Monitor")
                .font(FontId::new(13.0, FontFamily::Monospace))
                .color(Color32::from_rgb(180, 220, 200))
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:>5.1} HOST", host.host_fps.clamp(0.0, 999.9)))
                    .font(FontId::monospace(11.0))
                    .color(Color32::DARK_GRAY),
            );
        });
    });
    ui.horizontal(|ui| {
        let (dot, label, color) = if running {
            (
                Color32::from_rgb(80, 220, 120),
                "RUNNING",
                Color32::from_rgb(80, 220, 120),
            )
        } else {
            (Color32::DARK_GRAY, "IDLE", Color32::GRAY)
        };
        ui.colored_label(dot, "●");
        ui.label(
            RichText::new(label)
                .font(FontId::monospace(12.0))
                .color(color)
                .strong(),
        );
        ui.separator();
        ui.monospace(if host.rom_title.is_empty() {
            "—"
        } else {
            host.rom_title.as_str()
        });
        ui.separator();
        ui.label(
            RichText::new("DMG")
                .font(FontId::monospace(11.0))
                .color(Color32::from_rgb(100, 140, 180)),
        );
    });
}

pub(super) fn draw_capture_toolbar(
    ui: &mut Ui,
    frame: &mut DebugFrame<'_>,
) -> Option<MonitorAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let cap_label = if frame.capture_pending {
            "CAPTURING…"
        } else {
            "CAPTURE  F9"
        };
        if ui
            .add_enabled(
                !frame.capture_pending,
                egui::Button::new(RichText::new(cap_label).monospace()),
            )
            .on_hover_text("History capture: −15s … +5s around trigger")
            .clicked()
        {
            action = Some(MonitorAction::Capture);
        }
        if ui
            .button(RichText::new("COPY SNAPSHOT").monospace())
            .on_hover_text("Immediate current-state snapshot (no history)")
            .clicked()
        {
            action = Some(MonitorAction::CopySnapshot);
        }

        if frame.has_report {
            ui.separator();
            ui.label(
                RichText::new("CAPTURE READY")
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(80, 220, 120))
                    .strong(),
            );
            if ui.button(RichText::new("COPY").monospace()).clicked() {
                action = Some(MonitorAction::CopyReport);
            }
            if ui.button(RichText::new("SAVE…").monospace()).clicked() {
                action = Some(MonitorAction::SaveReport);
            }
            if ui.button(RichText::new("VIEW").monospace()).clicked() {
                *frame.show_report = !*frame.show_report;
                action = Some(MonitorAction::ViewReport);
            }
        }
    });

    if frame.capture_pending
        && let Some(rem) = frame.capture_remaining_secs
    {
        let total = 5.0_f32;
        let done = ((total - rem) / total).clamp(0.0, 1.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("CAPTURING")
                    .font(FontId::monospace(11.0))
                    .color(Color32::from_rgb(220, 180, 60)),
            );
            bar(ui, done, Color32::from_rgb(220, 180, 60), 160.0);
            ui.label(
                RichText::new(format!("{rem:.1}s remaining"))
                    .font(FontId::monospace(11.0))
                    .color(Color32::GRAY),
            );
        });
    }
    action
}

pub(super) fn draw_report_viewer(ui: &mut Ui, frame: &mut DebugFrame<'_>) -> Option<MonitorAction> {
    let mut action = None;
    let Some(report) = frame.last_report else {
        *frame.show_report = false;
        return None;
    };
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(18, 20, 22))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Last capture report")
                        .font(FontId::proportional(13.0))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Close").clicked() {
                        *frame.show_report = false;
                    }
                    if ui.small_button("Copy").clicked() {
                        action = Some(MonitorAction::CopyReport);
                    }
                });
            });
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(report)
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(200, 200, 200)),
                    );
                });
        });
    action
}
