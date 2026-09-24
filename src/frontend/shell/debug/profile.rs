//! Per-frame Instant timings for the Debug Monitor (frontend + core tick breakdown).

use graycart::TickProfile;
use std::time::Duration;

/// Wall-clock costs for one presented host frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameProfile {
    pub cpu_exec: Duration,
    pub tick_timer: Duration,
    pub tick_dma: Duration,
    pub tick_cart: Duration,
    pub tick_apu: Duration,
    pub tick_ppu: Duration,
    pub audio_submit: Duration,
    pub framebuffer: Duration,
    pub egui: Duration,
    pub gpu_present: Duration,
    pub debug_snapshot: Duration,
    pub debug_monitor: Duration,
    pub frame_total: Duration,
}

impl FrameProfile {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        cpu_exec: Duration,
        ticks: TickProfile,
        audio_submit: Duration,
        framebuffer: Duration,
        egui: Duration,
        gpu_present: Duration,
        debug_snapshot: Duration,
        debug_monitor: Duration,
        frame_total: Duration,
    ) -> Self {
        Self {
            cpu_exec,
            tick_timer: ticks.timer,
            tick_dma: ticks.dma,
            tick_cart: ticks.cart,
            tick_apu: ticks.apu,
            tick_ppu: ticks.ppu,
            audio_submit,
            framebuffer,
            egui,
            gpu_present,
            debug_snapshot,
            debug_monitor,
            frame_total,
        }
    }

    pub fn tick_total(self) -> Duration {
        self.tick_timer + self.tick_dma + self.tick_cart + self.tick_apu + self.tick_ppu
    }

    /// CPU decode/execute excluding Bus::tick hardware (approx).
    pub fn cpu_only(self) -> Duration {
        self.cpu_exec.saturating_sub(self.tick_total())
    }

    pub fn rows(self) -> [(&'static str, Duration); 12] {
        [
            ("CPU / execute*", self.cpu_only()),
            ("Timer::tick", self.tick_timer),
            ("OAM DMA", self.tick_dma),
            ("Cart tick", self.tick_cart),
            ("Apu::tick", self.tick_apu),
            ("Ppu::tick", self.tick_ppu),
            ("Audio submit", self.audio_submit),
            ("Framebuffer", self.framebuffer),
            ("egui (game)", self.egui),
            ("GPU present", self.gpu_present),
            ("Debug snapshot", self.debug_snapshot),
            ("Debug monitor", self.debug_monitor),
        ]
    }
}

/// Rolling avg / max for one subsystem (ms).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProfileStat {
    pub avg_ms: f64,
    pub max_ms: f64,
    pub pct: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ProfileSummary {
    pub rows: Vec<(&'static str, ProfileStat)>,
    pub frame_avg_ms: f64,
    pub budget_ms: f64,
}

impl ProfileSummary {
    pub fn from_last(profile: FrameProfile, budget_ms: f64) -> Self {
        let total = profile.frame_total.as_secs_f64() * 1000.0;
        let denom = total.max(0.001);
        let mut rows = Vec::new();
        for (name, d) in profile.rows() {
            let ms = d.as_secs_f64() * 1000.0;
            rows.push((
                name,
                ProfileStat {
                    avg_ms: ms,
                    max_ms: ms,
                    pct: ms / denom * 100.0,
                },
            ));
        }
        Self {
            rows,
            frame_avg_ms: total,
            budget_ms,
        }
    }
}
