//! Separate native Debug Monitor window (observation only).

mod capture;
mod history;
mod host;
mod profile;
mod ui;

use history::HistoryBuffers;
use ui::{DebugFrame, MonitorAction, draw_monitor};

pub use capture::{DiagnosticRing, build_snapshot_report};
pub use host::{HostMetrics, RuntimeAudioMetrics};
pub use profile::FrameProfile;
pub use ui::MonitorAction as DebugMonitorAction;

use egui::{ClippedPrimitive, Context, TexturesDelta, ViewportId};
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions, ScreenDescriptor};
use pixels::wgpu;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::fonts::install_departure_mono;
use super::runtime::HostDebug;
use super::settings::{MonitorSections, MonitorSettings};

const UI_PERIOD: Duration = Duration::from_millis(66); // ~15 Hz

pub struct DebugMonitor {
    window: Arc<Window>,
    window_id: WindowId,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: Context,
    egui_state: egui_winit::State,
    renderer: EguiRenderer,
    screen_descriptor: ScreenDescriptor,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
    last_ui: Instant,
    pub history: HistoryBuffers,
    cached_debug: Option<HostDebug>,
    cached_host: HostMetrics,
    capture_pending: bool,
    capture_remaining_secs: Option<f32>,
    has_report: bool,
    last_report: Option<String>,
    sections: MonitorSections,
    show_report: bool,
    profile_show_max: bool,
    pub last_action: Option<MonitorAction>,
    pub last_redraw_cost: Duration,
}

impl DebugMonitor {
    pub fn open(
        event_loop: &ActiveEventLoop,
        game: Option<&Window>,
        prefs: &MonitorSettings,
    ) -> Result<Self, String> {
        let width = prefs.width.clamp(640.0, 2400.0);
        let height = prefs.height.clamp(480.0, 1600.0);
        let mut attrs = Window::default_attributes()
            .with_title("Graycart // Machine Monitor")
            .with_inner_size(LogicalSize::new(width, height));

        if let (Some(x), Some(y)) = (prefs.x, prefs.y) {
            attrs = attrs.with_position(LogicalPosition::new(x, y));
        } else if let Some(pos) = place_relative_to_game(game, width, height) {
            attrs = attrs.with_position(pos);
        }

        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let window = Arc::new(window);
        let window_id = window.id();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("no wgpu adapter: {e}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("debug-monitor"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
            experimental_features: Default::default(),
        }))
        .map_err(|e| e.to_string())?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let scale = window.scale_factor() as f32;
        let egui_ctx = Context::default();
        install_departure_mono(&egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(12, 14, 16);
        visuals.window_fill = egui::Color32::from_rgb(12, 14, 16);
        egui_ctx.set_visuals(visuals);
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            ViewportId::from_hash_of("debug_monitor"),
            event_loop,
            Some(scale),
            None,
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = EguiRenderer::new(&device, format, RendererOptions::default());
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: scale,
        };

        Ok(Self {
            window,
            window_id,
            surface,
            device,
            queue,
            config,
            egui_ctx,
            egui_state,
            renderer,
            screen_descriptor,
            paint_jobs: Vec::new(),
            textures: TexturesDelta::default(),
            last_ui: Instant::now()
                .checked_sub(UI_PERIOD)
                .unwrap_or_else(Instant::now),
            history: HistoryBuffers::default(),
            cached_debug: None,
            cached_host: HostMetrics::default(),
            capture_pending: false,
            capture_remaining_secs: None,
            has_report: false,
            last_report: None,
            sections: prefs.sections.clone(),
            show_report: false,
            profile_show_max: false,
            last_action: None,
            last_redraw_cost: Duration::ZERO,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Persistable geometry + section state.
    pub fn export_settings(&self) -> MonitorSettings {
        let scale = self.window.scale_factor();
        let size = self.window.inner_size();
        let (x, y) = self
            .window
            .outer_position()
            .ok()
            .map(|p| (p.x as f64 / scale, p.y as f64 / scale))
            .map_or((None, None), |(x, y)| (Some(x), Some(y)));
        MonitorSettings {
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
            x,
            y,
            sections: self.sections.clone(),
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.config.width = size.width;
                    self.config.height = size.height;
                    self.surface.configure(&self.device, &self.config);
                    self.screen_descriptor.size_in_pixels = [size.width, size.height];
                }
                true
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.screen_descriptor.pixels_per_point = *scale_factor as f32;
                true
            }
            _ => {
                self.egui_state
                    .on_window_event(&self.window, event)
                    .consumed
            }
        }
    }

    pub fn update_cache(
        &mut self,
        debug: Option<HostDebug>,
        host: HostMetrics,
        capture_pending: bool,
        capture_remaining_secs: Option<f32>,
        has_report: bool,
        last_report: Option<String>,
    ) {
        self.history.push(
            host.frame_ms_avg as f32,
            host.render_ms_avg as f32,
            host.audio_queued as f32,
            host.resample_step as f32,
            host.emu_tcycles_per_sec as f32,
        );
        if let Some(d) = debug {
            self.cached_debug = Some(d);
        }
        self.cached_host = host;
        self.capture_pending = capture_pending;
        self.capture_remaining_secs = capture_remaining_secs;
        self.has_report = has_report;
        self.last_report = last_report;
    }

    pub fn maybe_request_redraw(&mut self) -> bool {
        if self.last_ui.elapsed() >= UI_PERIOD {
            self.window.request_redraw();
            true
        } else {
            false
        }
    }

    pub fn take_action(&mut self) -> Option<MonitorAction> {
        self.last_action.take()
    }

    pub fn redraw(&mut self) -> Result<(), String> {
        let t0 = Instant::now();
        self.last_ui = Instant::now();
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let debug = self.cached_debug.clone();
        let host = self.cached_host.clone();
        let history = self.history.clone();
        let capture_pending = self.capture_pending;
        let capture_remaining_secs = self.capture_remaining_secs;
        let has_report = self.has_report;
        let last_report = self.last_report.clone();
        let mut sections = self.sections.clone();
        let mut show_report = self.show_report;
        let mut profile_show_max = self.profile_show_max;
        let mut action = None;
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                let (sm83, arm_lines) = match debug.as_ref() {
                    Some(HostDebug::Sm83(m)) => (Some(m), None),
                    Some(HostDebug::Arm(lines)) => (None, Some(lines.as_slice())),
                    None => (None, None),
                };
                action = draw_monitor(
                    ui,
                    &mut DebugFrame {
                        machine: sm83,
                        arm_lines,
                        host: &host,
                        history: &history,
                        capture_pending,
                        capture_remaining_secs,
                        has_report,
                        last_report: last_report.as_deref(),
                        sections: &mut sections,
                        show_report: &mut show_report,
                        profile_show_max: &mut profile_show_max,
                    },
                );
            });
        });
        self.sections = sections;
        self.show_report = show_report;
        self.profile_show_max = profile_show_max;
        self.last_action = action;
        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(&self.window, output.platform_output);
        self.paint_jobs = self
            .egui_ctx
            .tessellate(output.shapes, self.screen_descriptor.pixels_per_point);

        let frame = loop {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t)
                | wgpu::CurrentSurfaceTexture::Suboptimal(t) => break t,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.device, &self.config);
                }
                wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {
                    self.last_redraw_cost = t0.elapsed();
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("debug monitor surface validation error".into());
                }
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("debug-monitor"),
            });

        for (id, image_delta) in &self.textures.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("debug-egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.055,
                                b: 0.06,
                                a: 1.0,
                            }),
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

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.last_redraw_cost = t0.elapsed();
        Ok(())
    }
}

/// Prefer right of game window when room exists; else offset down/right; else None (OS place).
fn place_relative_to_game(
    game: Option<&Window>,
    mon_w: f64,
    mon_h: f64,
) -> Option<LogicalPosition<f64>> {
    let game = game?;
    let scale = game.scale_factor();
    let pos = game.outer_position().ok()?;
    let size = game.outer_size();
    let gap = 12.0;

    let monitor = game.current_monitor()?;
    let mpos = monitor.position();
    let msize = monitor.size();

    // Work in physical pixels, then convert to logical for with_position.
    let right = pos.x + size.width as i32 + (gap * scale) as i32;
    let top = pos.y;
    let fits_right = right + (mon_w * scale) as i32 <= mpos.x + msize.width as i32;
    let fits_height = top + (mon_h * scale) as i32 <= mpos.y + msize.height as i32;

    let (px, py) = if fits_right && fits_height {
        (right, top)
    } else {
        // Offset down/right from game origin, clamped into monitor.
        let ox = pos.x + (48.0 * scale) as i32;
        let oy = pos.y + size.height as i32 + (gap * scale) as i32;
        let max_x = mpos.x + msize.width as i32 - (mon_w * scale) as i32;
        let max_y = mpos.y + msize.height as i32 - (mon_h * scale) as i32;
        if oy <= max_y {
            (ox.clamp(mpos.x, max_x.max(mpos.x)), oy)
        } else if fits_right {
            (right, top.clamp(mpos.y, max_y.max(mpos.y)))
        } else {
            return None;
        }
    };

    let logical = PhysicalPosition::new(px, py).to_logical(scale);
    Some(logical)
}
