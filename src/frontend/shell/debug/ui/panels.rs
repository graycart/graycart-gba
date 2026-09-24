//! Collapsible monitor panels (performance, audio, profile, hardware).

use super::DebugFrame;
use super::format::{
    fmt_audio_queue, fmt_count_capped, fmt_overview_fps, fmt_overview_ms, fmt_overview_pct,
    fmt_overview_queue_pct,
};
use super::widgets::{b, bar, channel, collapsing, mono_inline, mono_kv, reg_row, sparkline};
use egui::{Color32, FontId, RichText, Sense, Ui, Vec2};
use graycart::GameBoyButton;

#[derive(Clone, Copy)]
pub(super) enum PanelId {
    Performance,
    Audio,
    Profile,
    Cpu,
    Apu,
    Ppu,
    Input,
    Gba,
}

pub(super) fn visible_panels(frame: &DebugFrame<'_>) -> Vec<PanelId> {
    if frame.arm_lines.is_some() {
        return vec![
            PanelId::Performance,
            PanelId::Audio,
            PanelId::Profile,
            PanelId::Gba,
        ];
    }
    vec![
        PanelId::Performance,
        PanelId::Audio,
        PanelId::Profile,
        PanelId::Cpu,
        PanelId::Apu,
        PanelId::Ppu,
        PanelId::Input,
    ]
}

pub(super) fn draw_panel_grid(
    ui: &mut Ui,
    frame: &mut DebugFrame<'_>,
    panels: &[PanelId],
    cols: usize,
) {
    let gap = 8.0;
    let width = ui.available_width();
    let col_w = ((width - gap * (cols as f32 - 1.0).max(0.0)) / cols as f32).max(220.0);

    let mut row: Vec<PanelId> = Vec::new();
    for (i, &panel) in panels.iter().enumerate() {
        row.push(panel);
        let flush = row.len() == cols || i + 1 == panels.len();
        if flush {
            ui.horizontal_top(|ui| {
                for (j, p) in row.iter().enumerate() {
                    if j > 0 {
                        ui.add_space(gap);
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(col_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(col_w - 4.0);
                            draw_panel(ui, frame, *p);
                        },
                    );
                }
            });
            ui.add_space(gap);
            row.clear();
        }
    }
}

fn draw_panel(ui: &mut Ui, frame: &mut DebugFrame<'_>, id: PanelId) {
    match id {
        PanelId::Performance => {
            collapsing(ui, "PERFORMANCE", &mut frame.sections.performance, |ui| {
                let host = frame.host;
                mono_kv(
                    ui,
                    "CYCLES",
                    &format!("{:>5.3}M / 4.194M", host.emu_tcycles_per_sec / 1e6),
                );
                mono_kv(ui, "REALTIME", &fmt_overview_pct(host.realtime_pct()));
                mono_kv(ui, "EMU FPS", &fmt_overview_fps(host.emu_fps));
                mono_kv(ui, "FRAME", &fmt_overview_ms(host.frame_ms_avg));
                mono_kv(
                    ui,
                    "RANGE",
                    &format!(
                        "{:>5.2}–{:>5.2} ms",
                        host.frame_ms_min.clamp(0.0, 999.99),
                        host.frame_ms_max.clamp(0.0, 999.99)
                    ),
                );
                mono_kv(
                    ui,
                    "RENDER",
                    &format!(
                        "{:>5.2}/{:>5.2}",
                        host.render_ms_avg.clamp(0.0, 999.99),
                        host.render_ms_max.clamp(0.0, 999.99)
                    ),
                );
                mono_kv(ui, "SLEEP", &fmt_overview_ms(host.pace_sleep_ms));
                mono_kv(ui, "MISS", &fmt_count_capped(host.missed_frames));
                mono_kv(ui, "REDRAW", &fmt_count_capped(host.redraws));
                mono_kv(
                    ui,
                    "INPUT/S",
                    &format!("{:>6.1}", host.input_events_per_sec.clamp(0.0, 9999.9)),
                );
            })
        }
        PanelId::Audio => collapsing(ui, "AUDIO", &mut frame.sections.audio, |ui| {
            let host = frame.host;
            let fill = (host.audio_queue_pct / 100.0).clamp(0.0, 1.5) as f32 / 1.5;
            const BAR_W: f32 = 220.0;
            bar(ui, fill.min(1.0), Color32::from_rgb(80, 160, 255), BAR_W);
            mono_kv(
                ui,
                "BUFFER",
                &format!(
                    "{}  ({})",
                    fmt_audio_queue(host.audio_queued, host.audio_target),
                    fmt_overview_queue_pct(host.audio_queue_pct).trim()
                ),
            );
            mono_kv(
                ui,
                "EVENTS",
                &format!(
                    "u{} miss{} ov{}",
                    fmt_count_capped(host.audio_underrun_events).trim(),
                    fmt_count_capped(host.audio_missing_samples).trim(),
                    fmt_count_capped(host.audio_overruns).trim()
                ),
            );
            mono_kv(ui, "STEP", &format!("{:>7.5}", host.resample_step));
            mono_kv(
                ui,
                "RATE",
                &if host.audio_offline() {
                    "     — Hz".to_string()
                } else {
                    format!("{:>6} Hz", host.audio_sample_rate.min(999_999))
                },
            );
            mono_kv(
                ui,
                "LAYOUT",
                &if host.audio_offline() {
                    "—".to_string()
                } else {
                    format!(
                        "{}ch {}",
                        host.audio_channels,
                        if host.audio_buffer_size.is_empty() {
                            "?"
                        } else {
                            host.audio_buffer_size.as_str()
                        }
                    )
                },
            );
            mono_kv(
                ui,
                "DEVICE",
                &if host.audio_offline() {
                    host.audio_init_error
                        .as_deref()
                        .unwrap_or("unavailable")
                        .to_string()
                } else if host.audio_device.is_empty() {
                    "—".to_string()
                } else {
                    host.audio_device.clone()
                },
            );
            mono_kv(
                ui,
                "STREAM",
                &if host.audio_offline() {
                    "not initialized".to_string()
                } else {
                    format!(
                        "cb/s {:>6.1}",
                        if host.audio_elapsed_secs > 0.05 {
                            host.audio_callbacks as f64 / host.audio_elapsed_secs
                        } else {
                            0.0
                        }
                    )
                },
            );
            mono_kv(
                ui,
                "P/C",
                &format!(
                    "{}/{}",
                    fmt_count_capped(host.audio_produced).trim(),
                    fmt_count_capped(host.audio_consumed).trim()
                ),
            );
            mono_kv(
                ui,
                "PEAK",
                &format!("{:>5.2} / {:>5.2}", host.peak_l, host.peak_r),
            );
            sparkline(
                ui,
                &frame.history.audio_queue,
                host.audio_target.max(1) as f32,
                Color32::from_rgb(100, 160, 255),
            );
        }),
        PanelId::Profile => collapsing(ui, "PROFILE", &mut frame.sections.profile, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("metric")
                        .font(FontId::monospace(10.0))
                        .color(Color32::GRAY),
                );
                if ui
                    .selectable_label(!*frame.profile_show_max, RichText::new("Avg").monospace())
                    .clicked()
                {
                    *frame.profile_show_max = false;
                }
                if ui
                    .selectable_label(*frame.profile_show_max, RichText::new("Max").monospace())
                    .clicked()
                {
                    *frame.profile_show_max = true;
                }
            });
            ui.label(
                RichText::new("* CPU excludes Bus::tick")
                    .font(FontId::monospace(9.0))
                    .color(Color32::DARK_GRAY),
            );
            let show_max = *frame.profile_show_max;
            // Always paint every profile row — filtering near-zero entries made the
            // list jump as timings flickered across the threshold.
            for (name, stat) in &frame.host.profile_summary.rows {
                let ms = if show_max { stat.max_ms } else { stat.avg_ms };
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [108.0, 12.0],
                        egui::Label::new(
                            RichText::new(*name)
                                .font(FontId::monospace(10.0))
                                .color(Color32::LIGHT_GRAY),
                        ),
                    );
                    ui.label(RichText::new(format!("{ms:5.2}")).font(FontId::monospace(10.0)));
                    ui.label(
                        RichText::new(format!("{:4.1}%", stat.pct))
                            .font(FontId::monospace(10.0))
                            .color(Color32::GRAY),
                    );
                    let bar_w = 56.0;
                    let w = (stat.pct as f32 / 100.0).clamp(0.0, 1.0) * bar_w;
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(bar_w, 7.0), Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, Color32::from_gray(35));
                    let mut fill = rect;
                    fill.set_width(w);
                    ui.painter()
                        .rect_filled(fill, 1.0, Color32::from_rgb(200, 160, 60));
                    resp.on_hover_text(format!(
                        "avg {:.2} ms · max {:.2} ms · {:.1}% of frame",
                        stat.avg_ms, stat.max_ms, stat.pct
                    ));
                });
            }
            ui.separator();
            mono_kv(
                ui,
                "TOTAL",
                &format!(
                    "{:.2} / {:.2} ms",
                    frame.host.profile_summary.frame_avg_ms, frame.host.profile_summary.budget_ms
                ),
            );
        }),
        PanelId::Cpu => collapsing(ui, "CPU", &mut frame.sections.cpu, |ui| {
            if let Some(m) = frame.machine {
                let c = &m.cpu;
                reg_row(ui, "PC", c.pc, "SP", c.sp);
                reg_row(ui, "AF", c.af, "BC", c.bc);
                reg_row(ui, "DE", c.de, "HL", c.hl);
                mono_kv(
                    ui,
                    "FLG",
                    &format!("Z{}N{}H{}C{}", b(c.z), b(c.n), b(c.h), b(c.c)),
                );
                mono_kv(ui, "IME", if c.ime { "ON " } else { "OFF" });
                mono_kv(ui, "OP", &c.mnemonic);
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Apu => collapsing(ui, "APU", &mut frame.sections.apu, |ui| {
            if let Some(m) = frame.machine {
                let a = &m.apu;
                mono_kv(ui, "NR52", &format!("{:02X}", a.nr52));
                channel(ui, "CH1", &a.ch1);
                channel(ui, "CH2", &a.ch2);
                channel(ui, "CH3", &a.ch3);
                channel(ui, "CH4", &a.ch4);
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Ppu => collapsing(ui, "PPU / TIMER", &mut frame.sections.ppu, |ui| {
            if let Some(m) = frame.machine {
                let p = &m.ppu;
                ui.horizontal(|ui| {
                    mono_inline(ui, "LY", &format!("{}", p.ly));
                    mono_inline(ui, "MODE", &format!("{}", p.mode));
                });
                mono_kv(ui, "LCDC", &format!("{:02X}", p.lcdc));
                mono_kv(ui, "STAT", &format!("{:02X}", p.stat));
                mono_kv(ui, "M3", &format!("{}", p.mode3_len));
                mono_kv(ui, "FRAME", &format!("{}", p.frame_index));
                let t = &m.timer;
                mono_kv(
                    ui,
                    "TIM",
                    &format!("DIV={:02X} TIMA={:02X} TAC={:02X}", t.div, t.tima, t.tac),
                );
            } else {
                ui.monospace("(no ROM)");
            }
        }),
        PanelId::Input => collapsing(ui, "INPUT", &mut frame.sections.input, |ui| {
            mono_kv(
                ui,
                "RATE",
                &format!("{:.1} /s", frame.host.input_events_per_sec),
            );
            ui.horizontal(|ui| {
                if let Some(m) = frame.machine {
                    for bttn in GameBoyButton::ALL {
                        let on = m.input.is_pressed(bttn);
                        let label = match bttn {
                            GameBoyButton::Left => "←",
                            GameBoyButton::Up => "↑",
                            GameBoyButton::Right => "→",
                            GameBoyButton::Down => "↓",
                            GameBoyButton::A => "A",
                            GameBoyButton::B => "B",
                            GameBoyButton::Start => "ST",
                            GameBoyButton::Select => "SE",
                        };
                        let c = if on {
                            Color32::from_rgb(80, 220, 120)
                        } else {
                            Color32::from_gray(60)
                        };
                        ui.colored_label(c, RichText::new(label).monospace());
                    }
                    ui.monospace(format!("  P1={:02X}", m.input.p1));
                }
            });
            ui.label(
                RichText::new("F12 toggle · F9 capture")
                    .font(FontId::monospace(9.0))
                    .color(Color32::DARK_GRAY),
            );
        }),
        PanelId::Gba => collapsing(ui, "GBA MACHINE", &mut frame.sections.cpu, |ui| {
            if let Some(lines) = frame.arm_lines {
                for line in lines {
                    ui.label(RichText::new(line).font(FontId::monospace(10.0)));
                }
            } else {
                ui.monospace("(no ROM)");
            }
        }),
    }
}

pub(super) fn column_count(width: f32) -> usize {
    if width >= 980.0 {
        3
    } else if width >= 640.0 {
        2
    } else {
        1
    }
}
