use super::*;
use crate::frontend::shell::playback::SpeedPreset;
use std::time::{Duration, Instant};

#[test]
fn frame_duration_matches_dmg_refresh() {
    // 70224 / 4194304 s → ~16.742706 ms; ~59.7275 Hz.
    assert_eq!(FRAME_DURATION.as_nanos(), 16_742_706);
    let fps = target_fps();
    assert!((fps - 59.7275).abs() < 0.0001);
}

#[test]
fn unthrottled_pacer_never_sleeps() {
    let mut pacer = FramePacer::new(false);
    let start = Instant::now();
    for _ in 0..100 {
        pacer.after_present_ex(false);
    }
    assert!(start.elapsed() < Duration::from_millis(50));
}

#[test]
fn skip_sleep_does_not_block() {
    let mut pacer = FramePacer::new(true);
    pacer.after_present_ex(false); // arm deadline
    let start = Instant::now();
    for _ in 0..30 {
        pacer.after_present_ex(true); // catch-up path
    }
    assert!(start.elapsed() < Duration::from_millis(50));
}

#[test]
fn fps_counter_starts_empty() {
    let mut fps = FpsCounter::new();
    assert!(fps.tick().is_none());
}

#[test]
fn n4_period_is_base_div_4() {
    assert_eq!(
        HostScheduler::frame_period(SpeedPreset::X4),
        FRAME_DURATION / 4
    );
}

#[test]
fn catch_up_counts_frames_behind_deadline_capped() {
    let now = Instant::now();
    let mut sched = HostScheduler::new(true);
    let period = FRAME_DURATION;

    assert_eq!(sched.frames_to_catch_up(SpeedPreset::X1, now, 10), 1);

    sched.on_frame_presented(SpeedPreset::X1, now);

    let late = now + period * 5;
    assert_eq!(sched.frames_to_catch_up(SpeedPreset::X1, late, 10), 5);
    assert_eq!(sched.frames_to_catch_up(SpeedPreset::X1, late, 2), 2);
}

#[test]
fn wake_wait_until_when_running_throttled() {
    let now = Instant::now();
    let mut sched = HostScheduler::new(true);
    sched.on_frame_presented(SpeedPreset::X1, now);
    let deadline = now + FRAME_DURATION;

    match sched.wake(false, false, false, now) {
        HostWake::WaitUntil(d) => assert_eq!(d, deadline),
        other => panic!("expected WaitUntil, got {other:?}"),
    }
}

#[test]
fn wake_poll_when_unlimited() {
    let sched = HostScheduler::new(true);
    assert!(matches!(
        sched.wake(false, true, false, Instant::now()),
        HostWake::Poll
    ));
}

#[test]
fn wake_poll_when_audio_emergency_skip_sleep() {
    let now = Instant::now();
    let mut sched = HostScheduler::new(true);
    sched.on_frame_presented(SpeedPreset::X1, now);
    assert!(matches!(
        sched.wake(false, false, true, now),
        HostWake::Poll
    ));
}

#[test]
fn wake_wait_when_paused() {
    let sched = HostScheduler::new(true);
    assert!(matches!(
        sched.wake(true, false, false, Instant::now()),
        HostWake::Wait
    ));
    assert!(matches!(
        sched.wake(true, true, false, Instant::now()),
        HostWake::Wait
    ));
}
