//! Dense, scrollable Machine Monitor UI (btop-inspired).

mod format;
mod graphs;
mod header;
mod health;
mod panels;
mod widgets;

use super::super::settings::MonitorSections;
use super::history::HistoryBuffers;
use super::host::HostMetrics;
use egui::Ui;
use graphs::draw_graphs;
use header::{draw_capture_toolbar, draw_report_viewer, draw_status_banner, draw_title_row};
use health::draw_health_overview;
use panels::{column_count, draw_panel_grid, visible_panels};

pub struct DebugFrame<'a> {
    pub machine: Option<&'a graycart::MachineDebug>,
    /// ARM `gba-debug:` lines when a `.gba` is running.
    pub arm_lines: Option<&'a [String]>,
    pub host: &'a HostMetrics,
    pub history: &'a HistoryBuffers,
    pub capture_pending: bool,
    pub capture_remaining_secs: Option<f32>,
    pub has_report: bool,
    pub last_report: Option<&'a str>,
    pub sections: &'a mut MonitorSections,
    pub show_report: &'a mut bool,
    pub profile_show_max: &'a mut bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorAction {
    Capture,
    CopyReport,
    SaveReport,
    CopySnapshot,
    ViewReport,
}

pub fn draw_monitor(ui: &mut Ui, frame: &mut DebugFrame<'_>) -> Option<MonitorAction> {
    let mut action = None;
    let host = frame.host;

    // ── Fixed header ──────────────────────────────────────────────
    draw_status_banner(ui, host);
    draw_title_row(ui, frame);
    ui.add_space(2.0);
    action = action.or(draw_capture_toolbar(ui, frame));

    if *frame.show_report
        && let Some(report_action) = draw_report_viewer(ui, frame)
    {
        action = action.or(Some(report_action));
    }

    ui.add_space(4.0);
    ui.separator();

    // ── Scrollable body ───────────────────────────────────────────
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            draw_health_overview(ui, frame);
            ui.add_space(6.0);
            draw_graphs(ui, frame);
            ui.add_space(8.0);

            let width = ui.available_width();
            let cols = column_count(width);
            let panels = visible_panels(frame);
            draw_panel_grid(ui, frame, &panels, cols);
        });

    action
}

#[cfg(test)]
mod tests {
    use super::format::{
        fmt_audio_queue, fmt_count_capped, fmt_overview_fps, fmt_overview_ms, fmt_overview_pct,
        fmt_overview_queue_pct,
    };
    use super::health::audio_health;
    use crate::frontend::shell::debug::host::HostMetrics;

    #[test]
    fn overview_formats_have_stable_widths() {
        assert_eq!(fmt_overview_pct(9.9).len(), fmt_overview_pct(100.0).len());
        assert_eq!(
            fmt_overview_pct(99.64).len(),
            fmt_overview_pct(100.00).len()
        );
        assert_eq!(fmt_overview_fps(9.9).len(), fmt_overview_fps(59.73).len());
        assert_eq!(fmt_overview_ms(4.97).len(), fmt_overview_ms(36.96).len());
        assert_eq!(
            fmt_overview_queue_pct(9.0).len(),
            fmt_overview_queue_pct(252.0).len()
        );
        assert_eq!(fmt_count_capped(0).len(), fmt_count_capped(1_424).len());
        assert_eq!(fmt_count_capped(999_999).len(), "999999+".len());
        assert_eq!(
            fmt_audio_queue(9, 2400).len(),
            fmt_audio_queue(2394, 2400).len()
        );
    }

    #[test]
    fn zero_host_rate_is_audio_offline_not_healthy_queue() {
        let host = HostMetrics {
            audio_sample_rate: 0,
            audio_target: 0,
            audio_queue_pct: 0.0,
            audio_underrun_events: 0,
            ..HostMetrics::default()
        };
        let audio = audio_health(&host);
        assert_eq!(audio.label, "OFF ");
        assert!(host.audio_offline());
    }
}
