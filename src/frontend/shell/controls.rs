//! Native Configure Controls window (separate from the game / Machine Monitor).

use super::fonts::install_departure_mono;
use super::host_input::{InputFrontend, PollResult};
use super::settings::FrontendSettings;
use super::ui::input_config::draw_controls;
use egui::{ClippedPrimitive, Context, TexturesDelta, ViewportId};
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions, ScreenDescriptor};
use pixels::wgpu;
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

const DEFAULT_W: f64 = 480.0;
const DEFAULT_H: f64 = 640.0;

pub struct ControlsWindow {
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
}

impl ControlsWindow {
    pub fn open(event_loop: &ActiveEventLoop, game: Option<&Window>) -> Result<Self, String> {
        let mut attrs = Window::default_attributes()
            .with_title("Graycart // Configure Controls")
            .with_inner_size(LogicalSize::new(DEFAULT_W, DEFAULT_H));

        if let Some(game) = game
            && let Ok(pos) = game.outer_position()
        {
            let scale = game.scale_factor();
            let size = game.outer_size();
            // Place to the left of the game window when possible.
            let x = (pos.x as f64 / scale) - DEFAULT_W - 16.0;
            let y = pos.y as f64 / scale;
            attrs = attrs.with_position(LogicalPosition::new(x.max(0.0), y.max(0.0)));
            let _ = size;
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
            label: Some("controls-window"),
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
            ViewportId::from_hash_of("controls_window"),
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
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
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

    pub fn redraw(
        &mut self,
        settings: &mut FrontendSettings,
        input: &mut InputFrontend,
        poll: &PollResult,
    ) -> Result<(), String> {
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    draw_controls(ui, settings, input, poll);
                });
            });
        });
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
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("controls window surface validation error".into());
                }
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("controls-egui"),
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
                    label: Some("controls-egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.047,
                                g: 0.055,
                                b: 0.063,
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
        Ok(())
    }
}
