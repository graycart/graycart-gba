//! Health overview strip and severity helpers.

use super::super::host::HostMetrics;
use super::format::{fmt_overview_fps, fmt_overview_ms, fmt_overview_pct, fmt_overview_queue_pct};
use super::widgets::card;
use egui::{Color32, FontId, RichText, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Health {
    Good,
    Warn,
    Bad,
    Info,
}

pub(super) struct AudioHealth {
    pub label: &'static str,
    pub level: Health,
}

pub(super) fn draw_health_overview(ui: &mut Ui, frame: &super::DebugFrame<'_>) {
    let host = frame.host;
    let rt = host.realtime_pct();
    let audio = audio_health(host);

    card(ui, "OVERVIEW", |ui| {
        let avail = ui.available_width().max(1.0);
        let gap = 6.0;
        // Fixed footprints: label + value geometry never changes with digit width.
        let widths = [
            ("REALTIME", 0.22_f32),
            ("EMU FPS", 0.18),
            ("AUDIO", 0.16),
            ("QUEUE", 0.16),
            ("FRAME P99", 0.28),
        ];
        let total_frac: f32 = widths.iter().map(|(_, f)| *f).sum();
        let cell_gap_total = gap * (widths.len().saturating_sub(1) as f32);
        let usable = (avail - cell_gap_total).max(1.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (label, frac) in widths.iter() {
                let w = (usable * (frac / total_frac)).max(72.0);
                let (value, level) = match *label {
                    "REALTIME" => (fmt_overview_pct(rt), rt_level(rt)),
                    "EMU FPS" => (fmt_overview_fps(host.emu_fps), Health::Info),
                    "AUDIO" => (audio.label.to_string(), audio.level),
                    "QUEUE" => {
                        if host.audio_offline() {
                            ("  —  ".to_string(), Health::Bad)
                        } else {
                            (
                                fmt_overview_queue_pct(host.audio_queue_pct),
                                queue_level(host.audio_queue_pct),
                            )
                        }
                    }
                    "FRAME P99" => (
                        fmt_overview_ms(host.frame_ms_max),
                        frame_level(host.frame_ms_max),
                    ),
                    _ => (String::new(), Health::Info),
                };
                metric_cell(ui, w, label, &value, level);
            }
        });
    });
}

pub(super) fn audio_health(host: &HostMetrics) -> AudioHealth {
    if host.audio_offline() {
        return AudioHealth {
            label: "OFF ",
            level: Health::Bad,
        };
    }
    let underrun_active = host.audio_underrun_events > 0 && host.audio_queue_pct < 50.0;
    if underrun_active {
        AudioHealth {
            // Fixed 4-char status; color carries severity.
            label: "BAD ",
            level: Health::Bad,
        }
    } else {
        match queue_level(host.audio_queue_pct) {
            Health::Good => AudioHealth {
                label: "OK  ",
                level: Health::Good,
            },
            Health::Warn => AudioHealth {
                label: "WARN",
                level: Health::Warn,
            },
            Health::Bad => AudioHealth {
                label: "BAD ",
                level: Health::Bad,
            },
            Health::Info => AudioHealth {
                label: "OK  ",
                level: Health::Info,
            },
        }
    }
}

fn rt_level(pct: f64) -> Health {
    if pct >= 99.0 {
        Health::Good
    } else if pct >= 97.0 {
        Health::Warn
    } else {
        Health::Bad
    }
}

fn queue_level(pct: f64) -> Health {
    if (70.0..=130.0).contains(&pct) {
        Health::Good
    } else if (40.0..70.0).contains(&pct) || (130.0..=160.0).contains(&pct) {
        Health::Warn
    } else {
        Health::Bad
    }
}

fn frame_level(max_ms: f64) -> Health {
    if max_ms <= 18.0 {
        Health::Good
    } else if max_ms <= 28.0 {
        Health::Warn
    } else {
        Health::Bad
    }
}

pub(super) fn health_color(h: Health) -> Color32 {
    match h {
        Health::Good => Color32::from_rgb(80, 200, 120),
        Health::Warn => Color32::from_rgb(220, 180, 60),
        Health::Bad => Color32::from_rgb(220, 80, 80),
        Health::Info => Color32::from_rgb(100, 160, 220),
    }
}

fn metric_cell(ui: &mut Ui, width: f32, key: &str, val: &str, h: Health) {
    let height = 34.0;
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_min_size(Vec2::new(width, height));
            ui.set_max_width(width);
            ui.add_sized(
                [width, 12.0],
                egui::Label::new(
                    RichText::new(key)
                        .font(FontId::monospace(9.0))
                        .color(Color32::GRAY),
                )
                .truncate(),
            );
            ui.add_sized(
                [width, 16.0],
                egui::Label::new(
                    RichText::new(val)
                        .font(FontId::monospace(13.0))
                        .color(health_color(h))
                        .strong(),
                )
                .halign(egui::Align::RIGHT)
                .truncate(),
            );
        },
    );
}
