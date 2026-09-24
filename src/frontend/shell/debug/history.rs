//! Rolling sparkline history (~20–30 s @ 15 Hz).

const CAP: usize = 450; // 30 s × 15 Hz

#[derive(Debug, Clone)]
pub struct Sparkline {
    buf: Vec<f32>,
    head: usize,
    len: usize,
}

impl Default for Sparkline {
    fn default() -> Self {
        Self::new()
    }
}

impl Sparkline {
    pub fn new() -> Self {
        Self {
            buf: vec![0.0; CAP],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, v: f32) {
        self.buf[self.head] = v;
        self.head = (self.head + 1) % CAP;
        self.len = (self.len + 1).min(CAP);
    }

    pub fn samples(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.len);
        if self.len == 0 {
            return out;
        }
        let start = if self.len < CAP { 0 } else { self.head };
        for i in 0..self.len {
            out.push(self.buf[(start + i) % CAP]);
        }
        out
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryBuffers {
    pub frame_ms: Sparkline,
    pub render_ms: Sparkline,
    pub audio_queue: Sparkline,
    pub resample_step: Sparkline,
    pub emu_mhz: Sparkline,
}

impl HistoryBuffers {
    pub fn push(
        &mut self,
        frame_ms: f32,
        render_ms: f32,
        audio_queue: f32,
        resample_step: f32,
        emu_hz: f32,
    ) {
        self.frame_ms.push(frame_ms);
        self.render_ms.push(render_ms);
        self.audio_queue.push(audio_queue);
        self.resample_step.push(resample_step);
        self.emu_mhz.push(emu_hz / 1_000_000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_wraps_and_preserves_order() {
        let mut s = Sparkline::new();
        for i in 0..10 {
            s.push(i as f32);
        }
        let v = s.samples();
        assert_eq!(v.len(), 10);
        assert_eq!(v[0], 0.0);
        assert_eq!(v[9], 9.0);
    }
}
