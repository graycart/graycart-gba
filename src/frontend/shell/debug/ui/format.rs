//! Stable overview formats — digit-width changes must not reflow the panel.

pub(super) fn fmt_overview_pct(v: f64) -> String {
    format!("{:>7.2}%", v.clamp(0.0, 9999.99))
}

pub(super) fn fmt_overview_fps(v: f64) -> String {
    format!("{:>6.2}", v.clamp(0.0, 9999.99))
}

pub(super) fn fmt_overview_ms(v: f64) -> String {
    format!("{:>6.2} ms", v.clamp(0.0, 9999.99))
}

pub(super) fn fmt_overview_queue_pct(v: f64) -> String {
    if v >= 9999.0 {
        "9999+%".to_string()
    } else {
        format!("{:>4.0}%", v.clamp(0.0, 9998.0))
    }
}

pub(super) fn fmt_audio_queue(queued: usize, target: usize) -> String {
    let q = queued.min(999_999);
    let t = target.min(999_999);
    format!("{q:>6} / {t:<6}")
}

pub(super) fn fmt_count_capped(n: u64) -> String {
    if n > 999_999 {
        "999999+".to_string()
    } else {
        format!("{n:>7}")
    }
}
