//! Rolling diagnostic samples + Markdown capture report.

use super::host::HostMetrics;
use super::profile::FrameProfile;
use crate::frontend::shell::runtime::HostDebug;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CAP: usize = 450; // 30 s @ 15 Hz
const POST_CAPTURE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct TelemetrySample {
    pub at: Instant,
    pub host: HostMetrics,
    pub profile: FrameProfile,
}

#[derive(Debug)]
pub struct DiagnosticRing {
    samples: Vec<Option<TelemetrySample>>,
    head: usize,
    len: usize,
    events: Vec<(Instant, String)>,
    capture: Option<CaptureState>,
    pub last_report: Option<String>,
}

#[derive(Debug)]
struct CaptureState {
    trigger: Instant,
    title: String,
    rom_path: String,
    debug: Option<HostDebug>,
}

impl Default for DiagnosticRing {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticRing {
    pub fn new() -> Self {
        Self {
            samples: (0..CAP).map(|_| None).collect(),
            head: 0,
            len: 0,
            events: Vec::new(),
            capture: None,
            last_report: None,
        }
    }

    pub fn note_event(&mut self, msg: impl Into<String>) {
        self.events.push((Instant::now(), msg.into()));
        if self.events.len() > 200 {
            self.events.drain(0..self.events.len() - 200);
        }
    }

    pub fn push(&mut self, host: HostMetrics, profile: FrameProfile) {
        self.samples[self.head] = Some(TelemetrySample {
            at: Instant::now(),
            host,
            profile,
        });
        self.head = (self.head + 1) % CAP;
        self.len = (self.len + 1).min(CAP);

        if let Some(cap) = &self.capture
            && cap.trigger.elapsed() >= POST_CAPTURE
        {
            self.finish_capture();
        }
    }

    pub fn arm_capture(&mut self, title: String, rom_path: String, debug: Option<HostDebug>) {
        self.note_event("CAPTURE armed");
        self.capture = Some(CaptureState {
            trigger: Instant::now(),
            title,
            rom_path,
            debug,
        });
    }

    pub fn capture_pending(&self) -> bool {
        self.capture.is_some()
    }

    /// Seconds remaining in the post-trigger capture window, if armed.
    pub fn capture_remaining(&self) -> Option<Duration> {
        self.capture
            .as_ref()
            .map(|c| POST_CAPTURE.saturating_sub(c.trigger.elapsed()))
    }

    pub fn samples_ordered(&self) -> Vec<&TelemetrySample> {
        let mut out = Vec::with_capacity(self.len);
        if self.len == 0 {
            return out;
        }
        let start = if self.len < CAP { 0 } else { self.head };
        for i in 0..self.len {
            if let Some(s) = self.samples[(start + i) % CAP].as_ref() {
                out.push(s);
            }
        }
        out
    }

    fn finish_capture(&mut self) {
        let Some(cap) = self.capture.take() else {
            return;
        };
        let report = build_report(&cap, self);
        self.last_report = Some(report);
        self.note_event("CAPTURE ready");
    }
}

fn build_report(cap: &CaptureState, ring: &DiagnosticRing) -> String {
    let samples = ring.samples_ordered();
    let window_start = cap.trigger.checked_sub(Duration::from_secs(15));
    let window_end = cap.trigger + POST_CAPTURE;
    let window: Vec<&TelemetrySample> = samples
        .into_iter()
        .filter(|s| {
            let after_start = window_start.is_none_or(|ws| s.at >= ws);
            after_start && s.at <= window_end
        })
        .collect();

    let mut cycles: Vec<f64> = window.iter().map(|s| s.host.emu_tcycles_per_sec).collect();
    let mut fps: Vec<f64> = window.iter().map(|s| s.host.emu_fps).collect();
    let mut frame_ms: Vec<f64> = window.iter().map(|s| s.host.frame_ms_avg).collect();
    let mut queue: Vec<f64> = window.iter().map(|s| s.host.audio_queued as f64).collect();
    cycles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    fps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    frame_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    queue.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    let pct = |v: &[f64], p: f64| {
        if v.is_empty() {
            0.0
        } else {
            let i = ((v.len() as f64 - 1.0) * p).round() as usize;
            v[i.min(v.len() - 1)]
        }
    };

    let last = window.last().map(|s| &s.host);
    let mut out = String::new();
    push_report_header(
        &mut out,
        "Graycart Diagnostic Capture",
        &cap.title,
        &cap.rom_path,
    );
    out.push_str("capture_window: [-15s, +5s] around trigger\n");
    out.push_str(&format!("samples_in_window: {}\n\n", window.len()));

    out.push_str("## PERFORMANCE\n\n");
    out.push_str("target_cycles_per_sec: 4194304\n");
    out.push_str(&format!("avg_cycles_per_sec: {:.0}\n", avg(&cycles)));
    out.push_str(&format!(
        "min_cycles_per_sec: {:.0}\n",
        cycles.first().copied().unwrap_or(0.0)
    ));
    out.push_str(&format!(
        "realtime_pct: {:.2}\n",
        avg(&cycles) / 4_194_304.0 * 100.0
    ));
    out.push_str(&format!("avg_emu_fps: {:.3}\n", avg(&fps)));
    out.push_str(&format!("p95_frame_ms: {:.2}\n", pct(&frame_ms, 0.95)));
    out.push_str(&format!("p99_frame_ms: {:.2}\n", pct(&frame_ms, 0.99)));
    out.push_str(&format!(
        "max_frame_ms: {:.2}\n\n",
        frame_ms.last().copied().unwrap_or(0.0)
    ));

    out.push_str("## AUDIO\n\n");
    if let Some(h) = last {
        out.push_str(&format_audio_section(h));
        if !h.audio_offline() {
            out.push_str(&format!("avg_queue: {:.0}\n", avg(&queue)));
            out.push_str(&format!(
                "min_queue: {:.0}\n\n",
                queue.first().copied().unwrap_or(0.0)
            ));
        }
    }

    out.push_str("## HOST / PACE\n\n");
    if let Some(h) = last {
        out.push_str(&format!("vsync: {}\n", h.vsync));
        out.push_str(&format!("pacer: {}\n", h.pacer));
        out.push_str(&format!("pace_sleep_ms: {:.2}\n", h.pace_sleep_ms));
        out.push_str(&format!("missed_frames: {}\n", h.missed_frames));
        out.push_str(&format!(
            "input_events_per_sec: {:.1}\n\n",
            h.input_events_per_sec
        ));
    }

    out.push_str("## SUBSYSTEM PROFILE (last sample in window)\n\n");
    if let Some(s) = window.last() {
        let budget = 16.74;
        let total = s.profile.frame_total.as_secs_f64() * 1000.0;
        for (name, d) in s.profile.rows() {
            let ms = d.as_secs_f64() * 1000.0;
            let pct = ms / total.max(0.001) * 100.0;
            out.push_str(&format!("{name:<18} {ms:7.2} ms  {pct:5.1}%\n"));
        }
        out.push_str(&format!(
            "{:<18} {total:7.2} ms  budget {budget:.2} ms\n\n",
            "Frame total"
        ));
    }

    if let Some(debug) = &cap.debug {
        out.push_str("## MACHINE SNAPSHOT (at arm)\n\n");
        match debug {
            HostDebug::Sm83(m) => {
                out.push_str(&format!(
                    "PC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} IME={} HALT={}\n",
                    m.cpu.pc,
                    m.cpu.sp,
                    m.cpu.af,
                    m.cpu.bc,
                    m.cpu.de,
                    m.cpu.hl,
                    m.cpu.ime,
                    m.cpu.halted
                ));
                out.push_str(&format!(
                    "PPU LY={} mode={} LCDC={:02X} STAT={:02X} frame={}\n",
                    m.ppu.ly, m.ppu.mode, m.ppu.lcdc, m.ppu.stat, m.ppu.frame_index
                ));
                out.push_str(&format!(
                    "APU NR52={:02X} CH1={} CH2={} CH3={} CH4={}\n",
                    m.apu.nr52,
                    m.apu.ch1.active,
                    m.apu.ch2.active,
                    m.apu.ch3.active,
                    m.apu.ch4.active
                ));
                out.push_str(&format!(
                    "IE={:02X} IF={:02X} P1={:02X}\n\n",
                    m.interrupts.ie, m.interrupts.if_, m.input.p1
                ));
            }
            HostDebug::Arm(lines) => {
                for line in lines {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }

    out.push_str("## EVENTS\n\n");
    for (at, msg) in &ring.events {
        let rel = at
            .checked_duration_since(cap.trigger)
            .map(|d| format!("+{:.2}s", d.as_secs_f64()))
            .or_else(|| {
                cap.trigger
                    .checked_duration_since(*at)
                    .map(|d| format!("-{:.2}s", d.as_secs_f64()))
            })
            .unwrap_or_else(|| "?".into());
        out.push_str(&format!("{rel:>8}  {msg}\n"));
    }
    out.push('\n');
    out
}

/// Immediate lightweight snapshot (current host + machine only — no history window).
pub fn build_snapshot_report(
    title: &str,
    rom_path: &str,
    host: &HostMetrics,
    debug: Option<&HostDebug>,
) -> String {
    let mut out = String::new();
    push_report_header(&mut out, "Graycart Diagnostic Snapshot", title, rom_path);
    out.push_str("kind: snapshot (current state only)\n\n");

    out.push_str("## PERFORMANCE\n\n");
    out.push_str("target_cycles_per_sec: 4194304\n");
    out.push_str(&format!(
        "cycles_per_sec: {:.0}\n",
        host.emu_tcycles_per_sec
    ));
    out.push_str(&format!("realtime_pct: {:.2}\n", host.realtime_pct()));
    out.push_str(&format!("emu_fps: {:.3}\n", host.emu_fps));
    out.push_str(&format!("frame_ms_avg: {:.2}\n", host.frame_ms_avg));
    out.push_str(&format!("frame_ms_max: {:.2}\n\n", host.frame_ms_max));

    out.push_str("## AUDIO\n\n");
    out.push_str(&format_audio_section(host));

    out.push_str("## SUBSYSTEM PROFILE\n\n");
    for (name, stat) in &host.profile_summary.rows {
        out.push_str(&format!(
            "{name:<18} {:7.2} ms  {:5.1}%\n",
            stat.avg_ms, stat.pct
        ));
    }
    out.push('\n');

    if let Some(debug) = debug {
        out.push_str("## MACHINE\n\n");
        match debug {
            HostDebug::Sm83(m) => {
                out.push_str(&format!(
                    "PC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} IME={} HALT={}\n",
                    m.cpu.pc,
                    m.cpu.sp,
                    m.cpu.af,
                    m.cpu.bc,
                    m.cpu.de,
                    m.cpu.hl,
                    m.cpu.ime,
                    m.cpu.halted
                ));
                out.push_str(&format!(
                    "PPU LY={} mode={} LCDC={:02X} STAT={:02X} frame={}\n",
                    m.ppu.ly, m.ppu.mode, m.ppu.lcdc, m.ppu.stat, m.ppu.frame_index
                ));
                out.push_str(&format!(
                    "APU NR52={:02X} CH1={} CH2={} CH3={} CH4={}\n",
                    m.apu.nr52,
                    m.apu.ch1.active,
                    m.apu.ch2.active,
                    m.apu.ch3.active,
                    m.apu.ch4.active
                ));
                out.push_str(&format!(
                    "IE={:02X} IF={:02X} P1={:02X}\n\n",
                    m.interrupts.ie, m.interrupts.if_, m.input.p1
                ));
            }
            HostDebug::Arm(lines) => {
                for line in lines {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }
    out
}

fn format_audio_section(host: &HostMetrics) -> String {
    if host.audio_offline() {
        let device = if host.audio_device.is_empty() {
            "unavailable".to_string()
        } else {
            host.audio_device.clone()
        };
        let reason = host
            .audio_init_error
            .as_deref()
            .unwrap_or("host stream was not initialized (host_rate_hz=0)");
        format!(
            "status: OFFLINE\n\
             AUDIO        OFFLINE\n\
             HOST RATE    —\n\
             DEVICE       {device}\n\
             STREAM       not initialized\n\
             reason: {reason}\n\n"
        )
    } else {
        let elapsed = host.audio_elapsed_secs.max(0.001);
        format!(
            "status: ONLINE\n\
             host_rate_hz: {}\n\
             DEVICE       {}\n\
             STREAM       initialized\n\
             channels: {}\n\
             buffer_size: {}\n\
             target_queue: {}\n\
             queue: {} / {} ({:.0}%)\n\
             samples generated/sec: {:.0}\n\
             samples submitted/sec: {:.0}\n\
             callback invocations/sec: {:.1}\n\
             underrun_events: {}\n\
             missing_samples: {}\n\
             overruns_dropped: {}\n\
             resample_step: {:.5}\n\n",
            host.audio_sample_rate,
            host.audio_device,
            host.audio_channels,
            host.audio_buffer_size,
            host.audio_target,
            host.audio_queued,
            host.audio_target,
            host.audio_queue_pct,
            host.audio_produced as f64 / elapsed,
            host.audio_consumed as f64 / elapsed,
            host.audio_callbacks as f64 / elapsed,
            host.audio_underrun_events,
            host.audio_missing_samples,
            host.audio_overruns,
            host.resample_step
        )
    }
}

fn push_report_header(out: &mut String, title: &str, rom_title: &str, rom_path: &str) {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let version = env!("CARGO_PKG_VERSION");
    let commit = option_env!("GRAYCART_GIT_SHA").unwrap_or("unknown");
    #[cfg(debug_assertions)]
    let build = "debug";
    #[cfg(not(debug_assertions))]
    let build = "release";

    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&format!("Graycart {version}\n"));
    out.push_str(&format!("commit {commit}\n"));
    out.push_str(&format!("build {build}\n"));
    #[cfg(debug_assertions)]
    out.push_str("(DEBUG builds are not representative of play performance)\n");
    out.push_str(&format!("\ntimestamp_unix: {epoch}\n"));
    out.push_str(&format!("rom_title: {rom_title}\n"));
    out.push_str(&format!("rom_path: {rom_path}\n"));
}

#[cfg(test)]
mod tests {
    use super::{build_snapshot_report, format_audio_section};
    use crate::frontend::shell::debug::host::HostMetrics;

    #[test]
    fn snapshot_treats_zero_host_rate_as_offline() {
        let host = HostMetrics {
            audio_init_error: Some("no output device".into()),
            ..HostMetrics::default()
        };
        assert!(host.audio_offline());
        let body = format_audio_section(&host);
        assert!(body.contains("status: OFFLINE"));
        assert!(body.contains("STREAM       not initialized"));
        assert!(body.contains("HOST RATE    —"));
        assert!(!body.contains("host_rate_hz: 0"));
        let report = build_snapshot_report("t", "rom.gb", &host, None);
        assert!(report.contains("AUDIO        OFFLINE"));
    }

    #[test]
    fn online_audio_reports_host_rate_not_offline() {
        let host = HostMetrics {
            audio_sample_rate: 48_000,
            audio_target: 2400,
            audio_queued: 2000,
            audio_queue_pct: 90.0,
            audio_device: "Speakers".into(),
            audio_channels: 2,
            audio_buffer_size: "Fixed(960)".into(),
            audio_elapsed_secs: 1.0,
            audio_produced: 48_000,
            audio_consumed: 47_000,
            audio_callbacks: 100,
            ..HostMetrics::default()
        };
        let body = format_audio_section(&host);
        assert!(body.contains("status: ONLINE"));
        assert!(body.contains("host_rate_hz: 48000"));
        assert!(body.contains("STREAM       initialized"));
        assert!(body.contains("channels: 2"));
        assert!(body.contains("buffer_size: Fixed(960)"));
    }
}
