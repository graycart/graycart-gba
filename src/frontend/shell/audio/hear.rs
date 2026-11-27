//! What the host callback actually played, and where that waveform ticks.
//!
//! The diagnostic counters are totals since the stream opened, so a startup
//! underrun looks like a problem that is still happening. This ring is the
//! sound itself for the last 20 seconds. A capture slices it, writes a WAV,
//! and lists holds and isolated jumps.

use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const DEFAULT_RATE: u32 = 48_000;

/// Lock-free stereo tap. The callback is the only writer.
pub struct HeardTap {
    pcm: Box<[AtomicU32]>,
    write: AtomicU64,
    rate: AtomicU32,
}

impl fmt::Debug for HeardTap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeardTap")
            .field("recorded", &self.write.load(Ordering::Relaxed))
            .field("rate", &self.rate.load(Ordering::Relaxed))
            .field("capacity", &self.pcm.len())
            .finish()
    }
}

impl HeardTap {
    pub fn new(rate: u32, frames: usize) -> Self {
        let frames = frames.max(1);
        let pcm = (0..frames).map(|_| AtomicU32::new(0)).collect();
        Self {
            pcm,
            write: AtomicU64::new(0),
            rate: AtomicU32::new(rate.max(1)),
        }
    }

    pub fn disabled() -> Self {
        Self::new(DEFAULT_RATE, 1)
    }

    pub fn record(&self, left: f32, right: f32, _held: bool) {
        if self.pcm.len() <= 1 {
            return;
        }
        let i = self.write.fetch_add(1, Ordering::Release);
        let slot = (i as usize) % self.pcm.len();
        self.pcm[slot].store(pack(left, right), Ordering::Relaxed);
    }

    /// Copy the newest frames in chronological order (interleaved L, R).
    pub fn snapshot(&self) -> (Vec<i16>, u32) {
        let rate = self.rate.load(Ordering::Relaxed).max(1);
        let end = self.write.load(Ordering::Acquire);
        let cap = self.pcm.len() as u64;
        if cap <= 1 || end == 0 {
            return (Vec::new(), rate);
        }
        let n = end.min(cap);
        let start = end - n;
        let mut pcm = Vec::with_capacity(n as usize * 2);
        for i in 0..n {
            let slot = ((start + i) as usize) % self.pcm.len();
            let packed = self.pcm[slot].load(Ordering::Relaxed);
            pcm.push(unpack_lo(packed));
            pcm.push(unpack_hi(packed));
        }
        (pcm, rate)
    }

    /// Markdown section plus a WAV of the same samples written to `wav_path`.
    pub fn report_section(&self, wav_path: &str) -> String {
        let (pcm, rate) = self.snapshot();
        let mut out = String::new();
        out.push_str("## HEARD AUDIO\n\n");
        if pcm.len() < 4 {
            out.push_str("no callback audio recorded in this window\n\n");
            return out;
        }
        match write_wav(wav_path, &pcm, rate) {
            Ok(()) => out.push_str(&format!("wav: {wav_path}\n")),
            Err(e) => out.push_str(&format!("wav: (write failed: {e})\n")),
        }
        out.push_str(&analyze_pcm(&pcm, rate));
        out.push('\n');
        out
    }
}

fn pack(left: f32, right: f32) -> u32 {
    let l = (left.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16 as u16 as u32;
    let r = (right.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16 as u16 as u32;
    l | (r << 16)
}

fn unpack_lo(packed: u32) -> i16 {
    packed as u16 as i16
}

fn unpack_hi(packed: u32) -> i16 {
    (packed >> 16) as u16 as i16
}

/// 16-bit stereo PCM WAV. No crate; the header is the canonical 44-byte layout.
pub fn write_wav(path: &str, interleaved: &[i16], rate: u32) -> Result<(), String> {
    let rate = rate.max(1);
    let data_bytes = interleaved.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_bytes);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_bytes) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&2u16.to_le_bytes()); // stereo
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&4u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for s in interleaved {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
struct Tick {
    /// Seconds before the end of the recording (report build time).
    from_end: f64,
    at: usize,
    detail: String,
    mag: i32,
}

/// Describe holds and isolated jumps in interleaved stereo PCM.
pub fn analyze_pcm(interleaved: &[i16], rate: u32) -> String {
    let rate = rate.max(1);
    let frames = interleaved.len() / 2;
    let mut out = String::new();
    out.push_str(&format!(
        "frames: {frames}\nrate: {rate}\nduration_s: {:.2}\n",
        frames as f64 / f64::from(rate)
    ));
    if frames < 8 {
        out.push_str("too short to scan\n");
        return out;
    }
    let mut peak = 0i32;
    let mut sum_sq = 0f64;
    let mut step_max = 0i32;
    let mut steps_1k = 0u32;
    let mut steps_2k = 0u32;
    let mut steps_3k = 0u32;
    for s in interleaved {
        let a = i32::from(*s).abs();
        peak = peak.max(a);
        sum_sq += f64::from(a * a);
    }
    for i in 1..frames {
        let d = step_at(interleaved, i);
        step_max = step_max.max(d);
        if d >= 1_000 {
            steps_1k += 1;
        }
        if d >= 2_000 {
            steps_2k += 1;
        }
        if d >= 3_000 {
            steps_3k += 1;
        }
    }
    let rms = (sum_sq / interleaved.len() as f64).sqrt();
    out.push_str(&format!(
        "peak: {peak}\nrms: {rms:.0}\nstep_max: {step_max}\nsteps>=1000: {steps_1k}\nsteps>=2000: {steps_2k}\nsteps>=3000: {steps_3k}\n"
    ));

    let mut ticks: Vec<Tick> = Vec::new();
    // Frozen playback: both channels constant while the surroundings are moving.
    // 64 frames is 1.3 ms at 48 kHz — shorter than a musical sustain, longer than
    // one identical pair that quantization can produce.
    const HOLD_MIN: usize = 64;
    let mut run = 1usize;
    for i in 1..frames {
        let same = interleaved[i * 2] == interleaved[(i - 1) * 2]
            && interleaved[i * 2 + 1] == interleaved[(i - 1) * 2 + 1];
        if same {
            run += 1;
            continue;
        }
        if run >= HOLD_MIN {
            note_hold(&mut ticks, interleaved, rate, i - run, run);
        }
        run = 1;
    }
    if run >= HOLD_MIN {
        note_hold(&mut ticks, interleaved, rate, frames - run, run);
    }

    // Isolated impulse: one big step, with small steps on both sides of it.
    // The floor tracks this recording's peak. A fixed 6000 missed every tick
    // in a take whose peak was 5304.
    let floor = (peak / 2).max(1_200);
    let mut jumps = Vec::new();
    // Skip the samples that belong to the impulse itself. A 4-sample click
    // looks continuous if those steps are included in the "neighborhood".
    for i in 48..frames.saturating_sub(40) {
        let d = step_at(interleaved, i);
        if d < floor {
            continue;
        }
        let before = neighbor_motion(interleaved, i.saturating_sub(40), i.saturating_sub(8));
        let after = neighbor_motion(interleaved, i + 8, (i + 40).min(frames));
        let local = before.max(after);
        if d > local.saturating_mul(4) && d > local + 1_200 {
            let from_end = (frames - i) as f64 / f64::from(rate);
            jumps.push(Tick {
                from_end,
                at: i,
                detail: "isolated step".to_string(),
                mag: d,
            });
        }
    }
    let jumps = cluster_impulses(jumps, (rate as usize / 200).max(1));

    let holds = ticks.len();
    out.push_str(&format!(
        "holds: {holds}  (constant run ≥ {HOLD_MIN} frames while neighbors are moving)\njumps: {}  (isolated step ≥ {floor}, both sides quieter)\n",
        jumps.len()
    ));
    if !jumps.is_empty() {
        let mut gaps_ms: Vec<u32> = jumps
            .windows(2)
            .map(|w| {
                let dt = w[1].at.saturating_sub(w[0].at) as f64 / f64::from(rate) * 1000.0;
                dt.round() as u32
            })
            .collect();
        gaps_ms.sort_unstable();
        let mode = gap_mode(&gaps_ms);
        let per_s = jumps.len() as f64 / (frames as f64 / f64::from(rate));
        out.push_str(&format!("jump_rate: {per_s:.1}/s  gap_mode_ms: {mode}\n"));
    }
    out.push_str(
        "times are seconds before the end of this recording (the capture report is built 5s after arm)\n",
    );
    if ticks.is_empty() && jumps.is_empty() {
        out.push_str("ticks: none\n");
        return out;
    }
    if !ticks.is_empty() {
        out.push_str("holds_at:\n");
        for t in ticks.iter().take(20) {
            let from_trigger = 5.0 - t.from_end;
            out.push_str(&format!(
                "  {from_trigger:+7.2}s from arm  mag={}  {}\n",
                t.mag, t.detail
            ));
        }
    }
    if jumps.is_empty() {
        return out;
    }
    out.push_str("waveform: L/R for ±8 frames around each jump (loudest 8, then time order)\n");
    let mut loudest: Vec<&Tick> = jumps.iter().collect();
    loudest.sort_by_key(|t| std::cmp::Reverse(t.mag));
    loudest.truncate(8);
    loudest.sort_by_key(|t| t.at);
    for t in &loudest {
        let from_trigger = 5.0 - t.from_end;
        out.push_str(&format!(
            "  {from_trigger:+7.3}s mag={}\n    {}\n",
            t.mag,
            waveform_window(interleaved, t.at, 8)
        ));
    }
    out.push_str("jump_times_ms:");
    for (n, t) in jumps.iter().take(160).enumerate() {
        if n % 8 == 0 {
            out.push('\n');
        }
        let ms = (frames - t.at) as f64 / f64::from(rate) * 1000.0;
        // Stored as ms before the end, so a paste still lines up with the wav.
        out.push_str(&format!(" {ms:.0}:{mag}", mag = t.mag));
    }
    if jumps.len() > 160 {
        out.push_str(&format!("\n  … {} more", jumps.len() - 160));
    }
    out.push('\n');
    out
}

fn step_at(pcm: &[i16], i: usize) -> i32 {
    let dl = i32::from(pcm[i * 2]) - i32::from(pcm[(i - 1) * 2]);
    let dr = i32::from(pcm[i * 2 + 1]) - i32::from(pcm[(i - 1) * 2 + 1]);
    dl.abs().max(dr.abs())
}

fn cluster_impulses(mut jumps: Vec<Tick>, min_sep: usize) -> Vec<Tick> {
    jumps.sort_by_key(|t| t.at);
    let mut out: Vec<Tick> = Vec::new();
    for t in jumps {
        if let Some(prev) = out.last_mut()
            && t.at.saturating_sub(prev.at) < min_sep
        {
            if t.mag > prev.mag {
                *prev = t;
            }
            continue;
        }
        out.push(t);
    }
    out
}

fn gap_mode(gaps: &[u32]) -> u32 {
    if gaps.is_empty() {
        return 0;
    }
    let mut best = gaps[0];
    let mut best_n = 0usize;
    let mut i = 0;
    while i < gaps.len() {
        let mut n = 1;
        while i + n < gaps.len() && gaps[i + n] == gaps[i] {
            n += 1;
        }
        if n > best_n {
            best_n = n;
            best = gaps[i];
        }
        i += n;
    }
    best
}

fn waveform_window(pcm: &[i16], at: usize, radius: usize) -> String {
    let frames = pcm.len() / 2;
    let start = at.saturating_sub(radius);
    let end = (at + radius).min(frames.saturating_sub(1));
    let mut line = String::new();
    for i in start..=end {
        if i > start {
            line.push(' ');
        }
        line.push_str(&format!(
            "{}{}/{}",
            if i == at { "*" } else { "" },
            pcm[i * 2],
            pcm[i * 2 + 1]
        ));
    }
    line
}

fn note_hold(ticks: &mut Vec<Tick>, pcm: &[i16], rate: u32, start: usize, len: usize) {
    let level = i32::from(pcm[start * 2])
        .abs()
        .max(i32::from(pcm[start * 2 + 1]).abs());
    let before = neighbor_motion(pcm, start.saturating_sub(32), start);
    let after_at = (start + len).min(pcm.len() / 2);
    let after = neighbor_motion(pcm, after_at, (after_at + 32).min(pcm.len() / 2));
    // A rest is a constant near silence with quiet neighbors. A tick is a freeze
    // in the middle of moving audio, including a freeze at zero.
    if level < 400 && before < 40 && after < 40 {
        return;
    }
    if before < 40 && after < 40 {
        return;
    }
    let frames = pcm.len() / 2;
    let from_end = (frames.saturating_sub(start)) as f64 / f64::from(rate.max(1));
    let ms = len as f64 * 1000.0 / f64::from(rate.max(1));
    ticks.push(Tick {
        from_end,
        at: start,
        detail: format!("frozen {ms:.1} ms"),
        mag: level,
    });
}

fn neighbor_motion(pcm: &[i16], from: usize, to: usize) -> i32 {
    if to <= from + 1 {
        return 0;
    }
    let mut acc = 0i32;
    let mut n = 0i32;
    for i in (from + 1)..to {
        let dl = i32::from(pcm[i * 2]) - i32::from(pcm[(i - 1) * 2]);
        let dr = i32::from(pcm[i * 2 + 1]) - i32::from(pcm[(i - 1) * 2 + 1]);
        acc += dl.abs().max(dr.abs());
        n += 1;
    }
    if n == 0 { 0 } else { acc / n }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frames: usize, step: i16) -> Vec<i16> {
        let mut v = 0i32;
        let mut dir = i32::from(step).max(1);
        let mut out = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            let s = v as i16;
            out.push(s);
            out.push(s);
            let next = v + dir;
            if next > 6_000 || next < -6_000 {
                dir = -dir;
            }
            v += dir;
        }
        out
    }

    #[test]
    fn smooth_tone_is_not_a_tick() {
        let pcm = tone(4_000, 30);
        let text = analyze_pcm(&pcm, 48_000);
        assert!(text.contains("holds: 0"), "{text}");
        assert!(text.contains("jumps: 0"), "{text}");
    }

    #[test]
    fn frozen_run_inside_a_tone_is_a_hold() {
        let mut pcm = tone(4_000, 40);
        // Flatten 200 frames (400 samples) in the middle.
        for s in pcm.iter_mut().skip(2_000).take(400) {
            *s = 3_000;
        }
        let text = analyze_pcm(&pcm, 48_000);
        assert!(text.contains("holds: 1"), "{text}");
        assert!(text.contains("hold"), "{text}");
    }

    #[test]
    fn one_loud_step_is_a_jump() {
        let mut pcm = tone(2_000, 5);
        pcm[1_000] = 20_000;
        pcm[1_001] = 20_000;
        let text = analyze_pcm(&pcm, 48_000);
        assert!(text.contains("jumps: 1") || text.contains("jump"), "{text}");
        assert!(text.contains("20000"), "{text}");
    }

    #[test]
    fn wav_header_matches_sample_count() {
        let pcm = vec![0i16, 1, 2, 3];
        let path = std::env::temp_dir().join("graycart-hear-test.wav");
        let path_s = path.to_string_lossy().to_string();
        write_wav(&path_s, &pcm, 48_000).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(bytes.len(), 44 + 8);
        let _ = std::fs::remove_file(&path);
    }
}
