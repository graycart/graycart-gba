//! Play window: file picker, pixels present, egui debug panel, cpal output.
//! Compiled only into the binary when feature `frontend` is enabled.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use egui::{ClippedPrimitive, Context, TexturesDelta, ViewportId};
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions, ScreenDescriptor};
use pixels::{wgpu, Pixels, PixelsContext, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use graycart::{
    apply_fast, bus_from_cartridge, Cartridge, Cpu, ExecSession, HostHardwarePref, RunOutcome,
    SCREEN_HEIGHT as GB_H, SCREEN_WIDTH as GB_W,
};
use graycart_gba::compat::{AGB_A, AGB_B};
use graycart_gba::debug::{live_cpu_line, MachineDebug};
use graycart_gba::frontend::{
    choose_output, gba_framebuffer_to_rgba, machine_kind, resample_linear,
    sm83_framebuffer_to_rgba, AudioOutputChoice, MachineKind,
};
use graycart_gba::ppu::sprite_count;
use graycart_gba::Machine;

const GBA_W: u32 = 240;
const GBA_H: u32 = 160;
const SCALE: u32 = 3;
const GBA_PCM_HZ: u32 = 32_768;
const NAMED_FALLBACK: &str = "Speakers";

enum PlayMachine {
    Arm(Box<Machine>),
    Sm83 {
        cpu: Cpu,
        bus: Box<graycart::Bus>,
        session: ExecSession,
    },
}

struct AudioPump {
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
}

/// Open the file picker and run until the window closes.
pub fn run() -> Result<(), String> {
    // SM83 bus is larger than the default Windows main-thread stack.
    std::thread::Builder::new()
        .name("gba-play".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(run_on_thread)
        .map_err(|err| err.to_string())?
        .join()
        .unwrap_or_else(|_| Err("play thread panicked".into()))
}

fn run_on_thread() -> Result<(), String> {
    let path = pick_rom()?;
    let kind = machine_kind(&path).ok_or_else(|| {
        format!(
            "unsupported extension: {}",
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("(none)")
        )
    })?;
    let machine = load_machine(&path, kind)?;
    let (fb_w, fb_h) = match kind {
        MachineKind::Arm => (GBA_W, GBA_H),
        MachineKind::Sm83 => (GB_W as u32, GB_H as u32),
    };

    let event_loop = EventLoop::new().map_err(|err| err.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = PlayApp {
        path,
        machine: Some(machine),
        fb_w,
        fb_h,
        window: None,
        pixels: None,
        gui: None,
        audio: open_audio().ok().flatten(),
        rgba: vec![0u8; (fb_w * fb_h * 4) as usize],
        debug_lines: Vec::new(),
        last_frame: Instant::now(),
        exit_error: None,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|err| err.to_string())?;
    if let Some(err) = app.exit_error {
        return Err(err);
    }
    Ok(())
}

fn pick_rom() -> Result<PathBuf, String> {
    let file = rfd::FileDialog::new()
        .add_filter("Game Boy Advance / Game Boy", &["gba", "gb", "gbc"])
        .pick_file();
    file.ok_or_else(|| "no file selected".to_string())
}

fn load_machine(path: &Path, kind: MachineKind) -> Result<PlayMachine, String> {
    match kind {
        MachineKind::Arm => Ok(PlayMachine::Arm(Box::new(Machine::open(path)?))),
        MachineKind::Sm83 => {
            let cart = Cartridge::load(path).map_err(|err| err.to_string())?;
            let bus = bus_from_cartridge(cart, HostHardwarePref::Automatic)
                .map_err(|err| err.to_string())?;
            let mut bus = Box::new(bus);
            let mut cpu = Cpu::new();
            apply_fast(&mut cpu, &mut bus);
            cpu.a = AGB_A;
            cpu.b = AGB_B;
            Ok(PlayMachine::Sm83 {
                cpu,
                bus,
                session: ExecSession::new(),
            })
        }
    }
}

fn open_audio() -> Result<Option<AudioPump>, String> {
    let host = cpal::default_host();
    let devices: Vec<cpal::Device> = host
        .output_devices()
        .map_err(|err| err.to_string())?
        .collect();
    let names: Vec<String> = devices.iter().filter_map(|d| d.name().ok()).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let os_default = host.default_output_device().and_then(|d| d.name().ok());
    let choice = choose_output(None, os_default.as_deref(), NAMED_FALLBACK, &name_refs);
    let AudioOutputChoice::Device { name, .. } = choice else {
        return Ok(None);
    };
    let device = devices
        .into_iter()
        .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
        .ok_or_else(|| format!("audio device gone: {name}"))?;
    let config = device
        .default_output_config()
        .map_err(|err| err.to_string())?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let q = Arc::clone(&queue);
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), channels, q)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), channels, q)?,
        other => return Err(format!("unsupported sample format: {other:?}")),
    };
    stream.play().map_err(|err| err.to_string())?;
    Ok(Some(AudioPump {
        _stream: stream,
        queue,
        sample_rate,
    }))
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
                for frame in data.chunks_mut(channels) {
                    let left = q.pop_front().unwrap_or(0.0);
                    let right = q.pop_front().unwrap_or(left);
                    if let Some(slot) = frame.first_mut() {
                        *slot = T::from_sample(left);
                    }
                    if channels > 1 {
                        if let Some(slot) = frame.get_mut(1) {
                            *slot = T::from_sample(right);
                        }
                    }
                    for slot in frame.iter_mut().skip(2) {
                        *slot = T::from_sample(0.0);
                    }
                }
            },
            |_| {},
            None,
        )
        .map_err(|err| err.to_string())
}

struct PlayApp {
    path: PathBuf,
    machine: Option<PlayMachine>,
    fb_w: u32,
    fb_h: u32,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    gui: Option<Gui>,
    audio: Option<AudioPump>,
    rgba: Vec<u8>,
    debug_lines: Vec<String>,
    last_frame: Instant,
    exit_error: Option<String>,
}

struct Gui {
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: EguiRenderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
}

impl Gui {
    fn new(
        event_loop: &ActiveEventLoop,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &Pixels<'_>,
    ) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
        let egui_ctx = Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(scale_factor),
            None,
            Some(max_texture_size),
        );
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = EguiRenderer::new(
            pixels.device(),
            pixels.render_texture_format(),
            RendererOptions::default(),
        );
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures: TexturesDelta::default(),
        }
    }

    fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.egui_state.on_window_event(window, event).consumed
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    fn prepare(&mut self, window: &Window, lines: &[String]) {
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            egui::Panel::right("gba_debug")
                .default_size(360.0)
                .show_inside(ui, |ui| {
                    ui.heading("gba-debug");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for line in lines {
                            ui.monospace(line);
                        }
                    });
                });
        });
        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(window, output.platform_output);
        self.paint_jobs = self
            .egui_ctx
            .tessellate(output.shapes, self.screen_descriptor.pixels_per_point);
    }

    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext<'_>,
    ) {
        for (id, image_delta) in &self.textures.set {
            self.renderer
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );
        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.renderer
                .render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }
        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
    }
}

impl PlayApp {
    fn step_frame(&mut self) -> Result<(), String> {
        let Some(machine) = self.machine.as_mut() else {
            return Ok(());
        };
        match machine {
            PlayMachine::Arm(m) => {
                m.run_frames(1);
                let frame = m.frames_done().max(1);
                gba_framebuffer_to_rgba(&m.ppu.pixels, &mut self.rgba);
                self.debug_lines = arm_debug_lines(m, frame);
                if let Some(audio) = self.audio.as_ref() {
                    let pcm = m.bus.apu.drain_pcm();
                    push_resampled(&audio.queue, &pcm, GBA_PCM_HZ, audio.sample_rate);
                }
            }
            PlayMachine::Sm83 { cpu, bus, session } => {
                let before = session.frames;
                match session.run_frames(cpu, bus, before + 1) {
                    RunOutcome::FrameLimit { .. } => {}
                    RunOutcome::Fault(report) => return Err(format!("sm83: {report}")),
                }
                let frame = session.frames as u32;
                sm83_framebuffer_to_rgba(&bus.ppu.framebuffer, &mut self.rgba);
                let mut lines = MachineDebug::absent().summary_lines(frame.max(1));
                lines.insert(
                    0,
                    format!(
                        "gba-debug: machine=sm83 a={AGB_A:02X} b={AGB_B:02X} file={}",
                        self.path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?")
                    ),
                );
                self.debug_lines = lines;
                if let Some(audio) = self.audio.as_ref() {
                    bus.apu.set_output_sample_rate(audio.sample_rate);
                    let samples = bus.apu.take_samples();
                    let mut q = audio.queue.lock().unwrap_or_else(|e| e.into_inner());
                    for s in samples {
                        q.push_back(s.left);
                        q.push_back(s.right);
                    }
                    const CAP: usize = 48_000 * 2;
                    while q.len() > CAP {
                        q.pop_front();
                    }
                }
            }
        }
        Ok(())
    }

    fn present(&mut self) -> Result<(), String> {
        let (window, pixels, gui) = match (&self.window, &mut self.pixels, &mut self.gui) {
            (Some(w), Some(p), Some(g)) => (Arc::clone(w), p, g),
            _ => return Ok(()),
        };
        let frame = pixels.frame_mut();
        let n = frame.len().min(self.rgba.len());
        frame[..n].copy_from_slice(&self.rgba[..n]);
        gui.prepare(&window, &self.debug_lines);
        pixels
            .render_with(|encoder, render_target, context| {
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: render_target,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                // Game pixels via the default pixels pass, then egui on top.
                context.scaling_renderer.render(encoder, render_target);
                gui.render(encoder, render_target, context);
                Ok(())
            })
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn arm_debug_lines(machine: &Machine, frame: u32) -> Vec<String> {
    let debug = MachineDebug::absent();
    let mut summary = debug.summary_lines(frame);
    summary[0] = live_cpu_line(
        frame,
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
        machine.idle,
        machine.bus.halted,
        machine.bus.irq.ime(),
        machine.bus.irq.ie(),
        machine.bus.irq.iff(),
    );
    let mode = machine.bus.dispcnt() & 7;
    let sprites = sprite_count(machine.bus.oam());
    summary[1] = format!("gba-debug: ppu frame={frame} mode={mode} sprites={sprites}");
    summary[2] = machine.bus.dma_debug_line(frame);
    summary[7] = machine.bus.apu.health_line(frame, &machine.bus.timers);
    summary
}

fn push_resampled(queue: &Mutex<VecDeque<f32>>, pcm: &[(i16, i16)], in_rate: u32, out_rate: u32) {
    let resampled = resample_linear(pcm, in_rate, out_rate);
    let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
    for (l, r) in resampled {
        q.push_back(l as f32 / 32768.0);
        q.push_back(r as f32 / 32768.0);
    }
    const CAP: usize = 48_000 * 2;
    while q.len() > CAP {
        q.pop_front();
    }
}

impl ApplicationHandler for PlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let title = format!(
            "graycart-gba — {}",
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("play")
        );
        let size = LogicalSize::new((self.fb_w * SCALE) as f64, (self.fb_h * SCALE) as f64);
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(size);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                self.exit_error = Some(err.to_string());
                event_loop.exit();
                return;
            }
        };
        let win_size = window.inner_size();
        let surface = SurfaceTexture::new(win_size.width, win_size.height, Arc::clone(&window));
        let pixels = match Pixels::new(self.fb_w, self.fb_h, surface) {
            Ok(p) => p,
            Err(err) => {
                self.exit_error = Some(err.to_string());
                event_loop.exit();
                return;
            }
        };
        let gui = Gui::new(
            event_loop,
            win_size.width,
            win_size.height,
            window.scale_factor() as f32,
            &pixels,
        );
        self.window = Some(window);
        self.pixels = Some(pixels);
        self.gui = Some(gui);
        if let Err(err) = self.step_frame() {
            self.exit_error = Some(err);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let min = Duration::from_millis(16);
        if self.last_frame.elapsed() < min {
            return;
        }
        self.last_frame = Instant::now();
        if let Err(err) = self.step_frame() {
            self.exit_error = Some(err);
            event_loop.exit();
            return;
        }
        if let Err(err) = self.present() {
            self.exit_error = Some(err);
            event_loop.exit();
            return;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let (Some(window), Some(gui)) = (&self.window, &mut self.gui) {
            let _ = gui.handle_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(PlayMachine::Arm(m)) = self.machine.as_ref() {
                    let _ = m.flush_save();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(pixels) = &mut self.pixels {
                    let _ = pixels.resize_surface(size.width, size.height);
                }
                if let Some(gui) = &mut self.gui {
                    gui.resize(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(gui) = &mut self.gui {
                    gui.scale_factor(scale_factor);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.present() {
                    self.exit_error = Some(err);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}
