use super::{SharedCounters, bind_bus_host_rate, fill_f32, probe_audio_backends};
use graycart::{Cartridge, HOST_SAMPLE_RATE, HostHardwarePref, bus_from_cartridge};
use rtrb::RingBuffer;
use std::sync::atomic::Ordering;

fn tiny_rom() -> Vec<u8> {
    vec![0u8; 0x200]
}

#[test]
fn probe_audio_backends_names_hosts() {
    let dump = probe_audio_backends();
    assert!(
        dump.contains("audio backend probe:"),
        "probe dump should always start with a header, got {dump}"
    );
    assert!(
        dump.contains("available hosts:") || dump.contains("default host:"),
        "{dump}"
    );
}

#[test]
fn bind_host_rate_after_rom_load_reset_and_hardware_relaunch() {
    let cart = Cartridge::rom_only(tiny_rom());
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoy).expect("dmg bus");
    bind_bus_host_rate(&mut bus, Some(44_100));
    assert_eq!(bus.apu.output_sample_rate(), 44_100);

    bus.power_on_keep_battery();
    assert_eq!(
        bus.apu.output_sample_rate(),
        HOST_SAMPLE_RATE,
        "power-on rebuilds the APU at the default host-rate constant"
    );
    bind_bus_host_rate(&mut bus, Some(44_100));
    assert_eq!(bus.apu.output_sample_rate(), 44_100);

    let cart = Cartridge::rom_only(tiny_rom());
    let mut cgb = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("cgb bus");
    bind_bus_host_rate(&mut cgb, Some(48_000));
    assert_eq!(cgb.apu.output_sample_rate(), 48_000);
    cgb.power_on_keep_battery();
    bind_bus_host_rate(&mut cgb, Some(44_100));
    assert_eq!(cgb.apu.output_sample_rate(), 44_100);

    bind_bus_host_rate(&mut cgb, None);
    assert_eq!(
        cgb.apu.output_sample_rate(),
        44_100,
        "missing host audio must not clobber a previously bound rate"
    );
}

#[test]
fn surround_callback_consumes_one_stereo_frame_per_host_frame() {
    // Regression for PRO X 2 / 5.1 WASAPI: old fill popped one ring sample per
    // host channel and drained 3× too fast (underrun_events in the thousands).
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(64);
    for &(l, r) in &[(0.25f32, -0.5), (0.75, 0.125), (-0.25, 0.5)] {
        producer.push(l).unwrap();
        producer.push(r).unwrap();
    }
    let counters = SharedCounters::new();
    let mut out = [0.0f32; 18]; // 3 frames × 6 channels
    fill_f32(&mut out, 6, &mut consumer, &counters);

    assert_eq!(&out[0..6], &[0.25, -0.5, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(&out[6..12], &[0.75, 0.125, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(&out[12..18], &[-0.25, 0.5, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(consumer.slots(), 0, "all stereo frames should be consumed");
    assert_eq!(counters.consumed.load(Ordering::Relaxed), 3);
    assert_eq!(counters.missing_samples.load(Ordering::Relaxed), 0);
    assert_eq!(counters.underrun_events.load(Ordering::Relaxed), 0);
}

#[test]
fn stereo_callback_still_interleaves_lr() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(16);
    producer.push(0.5).unwrap();
    producer.push(-0.5).unwrap();
    let counters = SharedCounters::new();
    let mut out = [0.0f32; 2];
    fill_f32(&mut out, 2, &mut consumer, &counters);
    assert_eq!(out, [0.5, -0.5]);
    assert_eq!(counters.consumed.load(Ordering::Relaxed), 1);
}

#[test]
fn mono_callback_downmixes() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(16);
    producer.push(0.5).unwrap();
    producer.push(-0.25).unwrap();
    let counters = SharedCounters::new();
    let mut out = [0.0f32; 1];
    fill_f32(&mut out, 1, &mut consumer, &counters);
    assert!((out[0] - 0.125).abs() < 1e-6);
    assert_eq!(counters.consumed.load(Ordering::Relaxed), 1);
}

#[test]
fn surround_underrun_counts_host_frames_not_channels() {
    let (_producer, mut consumer) = RingBuffer::<f32>::new(16);
    let counters = SharedCounters::new();
    let mut out = [1.0f32; 12]; // 2 empty frames × 6ch
    fill_f32(&mut out, 6, &mut consumer, &counters);
    assert!(out.iter().all(|&s| s == 0.0));
    assert_eq!(counters.consumed.load(Ordering::Relaxed), 0);
    assert_eq!(counters.missing_samples.load(Ordering::Relaxed), 2);
    assert_eq!(counters.underrun_events.load(Ordering::Relaxed), 1);
}

#[test]
fn channel_preference_ranks_stereo_before_surround() {
    use super::init::{channel_preference, rate_preference};
    assert!(channel_preference(2) < channel_preference(1));
    assert!(channel_preference(1) < channel_preference(6));
    assert!(channel_preference(2) < channel_preference(8));
    assert!(rate_preference(48_000) < rate_preference(44_100));
    assert!(rate_preference(44_100) < rate_preference(12_000));
}
