//! CPAL device discovery and stream construction. Failures are explicit strings.

use super::os_default::os_default_render_endpoint_names;
use super::select::{
    AudioDevicePref, AudioDeviceSource, AudioOutputChoice, choose_output_device,
    should_persist_choice,
};
use super::{AudioOut, SharedCounters, fill_f32, fill_f64, fill_i16, fill_i32, fill_u16};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    Device, Host, SampleFormat, SampleRate, Stream, StreamConfig, SupportedStreamConfig,
    SupportedStreamConfigRange,
};
use rtrb::RingBuffer;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use super::resample::AdaptiveResampler;

#[derive(Debug, Clone)]
pub struct AudioInitError {
    pub message: String,
    pub probe: String,
}

impl std::fmt::Display for AudioInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.probe.is_empty() {
            write!(f, "\n{}", self.probe)?;
        }
        Ok(())
    }
}

/// Human-readable inventory of CPAL hosts / output devices (always safe to call).
pub fn probe_audio_backends() -> String {
    let mut out = String::from("audio backend probe:\n");
    let hosts = cpal::available_hosts();
    if hosts.is_empty() {
        out.push_str("  available hosts: (none)\n");
        return out;
    }
    out.push_str(&format!(
        "  available hosts: {}\n",
        hosts
            .iter()
            .map(|h| format!("{h:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let default_id = cpal::default_host().id();
    out.push_str(&format!("  default host: {default_id:?}\n"));
    for id in hosts {
        let host = match cpal::host_from_id(id) {
            Ok(h) => h,
            Err(e) => {
                out.push_str(&format!("  host {id:?}: open failed ({e})\n"));
                continue;
            }
        };
        describe_host(&mut out, &host, id);
    }
    out
}

fn describe_host(out: &mut String, host: &Host, id: cpal::HostId) {
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "(none)".into());
    out.push_str(&format!("  host {id:?}: default output = {default_name}\n"));
    match host.output_devices() {
        Ok(list) => {
            let mut n = 0usize;
            for dev in list {
                n += 1;
                let name = dev.name().unwrap_or_else(|_| "(unnamed)".into());
                match dev.default_output_config() {
                    Ok(cfg) => out.push_str(&format!(
                        "    device {n}: {name}  {}ch {} Hz {:?}\n",
                        cfg.channels(),
                        cfg.sample_rate().0,
                        cfg.sample_format()
                    )),
                    Err(e) => {
                        out.push_str(&format!("    device {n}: {name}  default config: {e}\n"))
                    }
                }
            }
            if n == 0 {
                out.push_str("    (no output devices)\n");
            }
        }
        Err(e) => out.push_str(&format!("    output_devices() failed: {e}\n")),
    }
}

pub fn open_output_with_pref(pref: &AudioDevicePref) -> Result<AudioOut, AudioInitError> {
    let probe = probe_audio_backends();
    let os_defaults = os_default_render_endpoint_names();
    let os_refs: Vec<&str> = os_defaults.iter().map(String::as_str).collect();
    let mut last = String::new();
    let mut hosts: Vec<(String, Host)> = Vec::new();
    let default = cpal::default_host();
    let default_name = format!("{:?}", default.id());
    hosts.push((default_name, default));
    for id in cpal::available_hosts() {
        if hosts.iter().any(|(_, h)| h.id() == id) {
            continue;
        }
        match cpal::host_from_id(id) {
            Ok(h) => hosts.push((format!("{id:?}"), h)),
            Err(e) => last = format!("host {id:?}: {e}"),
        }
    }

    for (host_name, host) in hosts {
        match try_host_with_pref(&host, &host_name, pref, &os_refs) {
            Ok(audio) => return Ok(audio),
            Err(e) => last = e,
        }
    }
    Err(AudioInitError {
        message: format!(
            "audio backend initialized: no\nSTREAM not initialized\nlast error: {last}"
        ),
        probe,
    })
}

fn collect_output_devices(host: &Host) -> Vec<Device> {
    let mut devices: Vec<Device> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(d) = host.default_output_device() {
        if let Ok(name) = d.name() {
            seen.insert(name);
        }
        devices.push(d);
    }
    if let Ok(list) = host.output_devices() {
        for d in list {
            if let Ok(name) = d.name()
                && !seen.insert(name)
            {
                continue;
            }
            devices.push(d);
        }
    }
    devices
}

pub fn list_output_device_names() -> Vec<String> {
    let host = cpal::default_host();
    collect_output_devices(&host)
        .into_iter()
        .filter_map(|d| d.name().ok())
        .collect()
}

fn try_host_with_pref(
    host: &Host,
    host_name: &str,
    pref: &AudioDevicePref,
    os_defaults: &[&str],
) -> Result<AudioOut, String> {
    let mut devices = collect_output_devices(host);
    if devices.is_empty() {
        return Err(format!(
            "host {host_name}: default output device unavailable (no enumerated outputs)"
        ));
    }
    let names: Vec<String> = devices.iter().filter_map(|d| d.name().ok()).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let cpal_default = host.default_output_device().and_then(|d| d.name().ok());
    let choice = choose_output_device(pref, cpal_default.as_deref(), os_defaults, &name_refs);
    let (wanted, source) = match choice {
        AudioOutputChoice::Offline => {
            return Err(format!("host {host_name}: no output devices"));
        }
        AudioOutputChoice::Device { name, source } => (name, source),
    };

    let Some(idx) = devices
        .iter()
        .position(|d| d.name().ok().as_deref() == Some(wanted.as_str()))
    else {
        return Err(format!(
            "host {host_name}: selected output {wanted:?} was not openable"
        ));
    };
    let device = devices.swap_remove(idx);
    match try_device(device, host_name, source) {
        Ok(a) => {
            if !should_persist_choice(source) {
                eprintln!(
                    "audio: opened {} via {:?} (config negotiated)",
                    a.device_name, source
                );
            }
            Ok(a)
        }
        Err(e) => Err(e),
    }
}

/// Preferred host rates to probe inside each supported range (WASAPI shared-mode
/// endpoints often reject an arbitrary max rate even when the range advertises it).
const PREFERRED_RATES_HZ: &[u32] = &[48_000, 44_100, 96_000, 32_000, 22_050, 16_000];

pub(crate) fn channel_preference(channels: u16) -> u8 {
    // Stereo first (our ring is L/R), then mono, then surround/multi.
    // Do not let a 5.1/7.1 WASAPI default win before a working stereo mix —
    // gaming headsets (e.g. PRO X 2 LIGHTSPEED) often advertise surround first.
    match channels {
        2 => 0,
        1 => 1,
        _ => 2,
    }
}

pub(crate) fn rate_preference(hz: u32) -> u8 {
    PREFERRED_RATES_HZ
        .iter()
        .position(|&r| r == hz)
        .map(|i| i as u8)
        .unwrap_or(PREFERRED_RATES_HZ.len() as u8)
}

/// Expand supported ranges into concrete configs: prefer ≤2ch, then common rates.
///
/// Multi-channel defaults are still attempted (stream open may require them) but
/// only after stereo/mono candidates, so surround endpoints do not starve stereo.
pub(crate) fn expand_supported_configs(
    default: Option<SupportedStreamConfig>,
    ranges: impl IntoIterator<Item = SupportedStreamConfigRange>,
) -> Vec<SupportedStreamConfig> {
    let ranges: Vec<_> = ranges.into_iter().collect();
    let mut configs: Vec<SupportedStreamConfig> = Vec::new();
    let mut push_unique = |cfg: SupportedStreamConfig| {
        if !configs.iter().any(|c| {
            c.channels() == cfg.channels()
                && c.sample_format() == cfg.sample_format()
                && c.sample_rate() == cfg.sample_rate()
        }) {
            configs.push(cfg);
        }
    };

    if let Some(c) = default {
        push_unique(c);
    }

    for range in ranges {
        for &hz in PREFERRED_RATES_HZ {
            if let Some(cfg) = range.try_with_sample_rate(SampleRate(hz)) {
                push_unique(cfg);
            }
        }
        let min = range.min_sample_rate();
        push_unique(range.with_sample_rate(min));
        push_unique(range.with_max_sample_rate());
    }

    configs.sort_by_key(|c| {
        (
            channel_preference(c.channels()),
            rate_preference(c.sample_rate().0),
            format!("{:?}", c.sample_format()),
        )
    });
    configs
}

fn try_device(
    device: Device,
    host_name: &str,
    source: AudioDeviceSource,
) -> Result<AudioOut, String> {
    let name = device
        .name()
        .unwrap_or_else(|_| "unknown device".to_string());
    let default = device.default_output_config().ok();
    let ranges = device
        .supported_output_configs()
        .map(|it| it.collect::<Vec<_>>())
        .unwrap_or_default();
    if default.is_none() && ranges.is_empty() {
        return Err(format!(
            "{host_name}/{name}: no default or supported output configs"
        ));
    }
    let configs = expand_supported_configs(default, ranges);
    let mut last = String::new();
    for supported in configs {
        match try_config(&device, &name, host_name, supported, source) {
            Ok(a) => return Ok(a),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn try_config(
    device: &Device,
    name: &str,
    host_name: &str,
    supported: SupportedStreamConfig,
    source: AudioDeviceSource,
) -> Result<AudioOut, String> {
    let sample_format = supported.sample_format();
    let sample_rate = supported.sample_rate().0;
    // Keep the channel count from the negotiated SupportedStreamConfig.
    // Forcing stereo on a multi-channel WASAPI mix format is a common
    // "requested stream configuration is not supported" failure on Windows.
    // Callbacks map our stereo ring into N-channel frames (see fill_interleaved).
    let mut config: StreamConfig = supported.into();
    let channels = config.channels;

    // Soft target ~5 display frames (~83 ms @ 48 kHz): margin for USB /
    // wireless headsets whose WASAPI Default period often lands near ~10 ms.
    let target_frames = ((sample_rate as usize) / 60).saturating_mul(5).max(1024);
    let max_frames = ((sample_rate as usize) * 3 / 20).max(target_frames * 2);
    let capacity_samples = max_frames * 2;

    // Prefer ~20 ms device periods when the host honors Fixed; fall back below.
    let preferred_period = (sample_rate / 50).max(256);
    config.buffer_size = cpal::BufferSize::Fixed(preferred_period);

    let (producer, consumer) = RingBuffer::<f32>::new(capacity_samples);
    let consumer = Arc::new(Mutex::new(consumer));
    let counters = Arc::new(SharedCounters::new());
    let mut buffer_size = format!("Fixed({preferred_period})");
    let stream = match build_stream(
        device,
        &config,
        sample_format,
        channels,
        Arc::clone(&consumer),
        Arc::clone(&counters),
    ) {
        Ok(s) => s,
        Err(_) => {
            config.buffer_size = cpal::BufferSize::Default;
            buffer_size = "Default".to_string();
            build_stream(
                device,
                &config,
                sample_format,
                channels,
                Arc::clone(&consumer),
                Arc::clone(&counters),
            )?
        }
    };

    stream
        .play()
        .map_err(|e| format!("{host_name}/{name}: stream.play() failed ({e})"))?;

    let mut producer = producer;
    let prime = (target_frames * 85 / 100).min(max_frames) * 2;
    for _ in 0..prime {
        let _ = producer.push(0.0);
    }

    Ok(AudioOut {
        _stream: stream,
        producer: RefCell::new(producer),
        consumer,
        capacity_samples,
        counters,
        resampler: RefCell::new(AdaptiveResampler::new()),
        started: Instant::now(),
        sample_rate,
        target_frames,
        device_name: name.to_string(),
        host_name: host_name.to_string(),
        sample_format: format!("{sample_format:?}"),
        channels,
        buffer_size,
        device_source: source,
    })
}

fn build_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    channels: u16,
    consumer: Arc<Mutex<rtrb::Consumer<f32>>>,
    counters: Arc<SharedCounters>,
) -> Result<Stream, String> {
    let channels = channels as usize;
    let result = match sample_format {
        SampleFormat::F32 => {
            let consumer = Arc::clone(&consumer);
            let ctr = Arc::clone(&counters);
            device.build_output_stream(
                config,
                move |data: &mut [f32], _| {
                    if let Ok(mut ring) = consumer.lock() {
                        fill_f32(data, channels, &mut ring, &ctr);
                    }
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
        }
        SampleFormat::I16 => {
            let consumer = Arc::clone(&consumer);
            let ctr = Arc::clone(&counters);
            device.build_output_stream(
                config,
                move |data: &mut [i16], _| {
                    if let Ok(mut ring) = consumer.lock() {
                        fill_i16(data, channels, &mut ring, &ctr);
                    }
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
        }
        SampleFormat::I32 => {
            let consumer = Arc::clone(&consumer);
            let ctr = Arc::clone(&counters);
            device.build_output_stream(
                config,
                move |data: &mut [i32], _| {
                    if let Ok(mut ring) = consumer.lock() {
                        fill_i32(data, channels, &mut ring, &ctr);
                    }
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
        }
        SampleFormat::U16 => {
            let consumer = Arc::clone(&consumer);
            let ctr = Arc::clone(&counters);
            device.build_output_stream(
                config,
                move |data: &mut [u16], _| {
                    if let Ok(mut ring) = consumer.lock() {
                        fill_u16(data, channels, &mut ring, &ctr);
                    }
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
        }
        SampleFormat::F64 => {
            let consumer = Arc::clone(&consumer);
            let ctr = Arc::clone(&counters);
            device.build_output_stream(
                config,
                move |data: &mut [f64], _| {
                    if let Ok(mut ring) = consumer.lock() {
                        fill_f64(data, channels, &mut ring, &ctr);
                    }
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
        }
        other => {
            return Err(format!(
                "unsupported sample format {other:?} (need F32/I16/I32/U16/F64)"
            ));
        }
    };
    result.map_err(|e| format!("CPAL stream created: no ({e})"))
}
