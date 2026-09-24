//! Phase 9 — thin egui shell over the pixels presenter.

pub(crate) mod input_config;
mod menu;
mod palette_editor;
mod rom_picker;

use egui::{
    ClippedPrimitive, ColorImage, Context, TextureHandle, TextureOptions, TexturesDelta, ViewportId,
};
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions, ScreenDescriptor};
use menu::{RuntimeUi, menu_bar};
use palette_editor::palette_editor_window;
use pixels::{PixelsContext, wgpu};
use rom_picker::empty_rom_screen;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::fonts::install_departure_mono;
use super::playback::SpeedPreset;
use super::report::{ConsentOutcome, ReportUi, show as show_report_ui};
use super::settings::FrontendSettings;
use super::video::{DisplayMode, game_image_size};

pub use menu::UiAction;

/// egui state layered on top of `pixels`.
pub struct Gui {
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: EguiRenderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
    game_texture: Option<TextureHandle>,
    pub runtime: RuntimeUi,
    pub report: ReportUi,
    pending: Vec<UiAction>,
    last_consent: ConsentOutcome,
}

impl Gui {
    pub fn new(
        event_loop: &ActiveEventLoop,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &pixels::Pixels<'_>,
    ) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
        let egui_ctx = Context::default();
        install_departure_mono(&egui_ctx);
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
            game_texture: None,
            runtime: RuntimeUi::default(),
            report: ReportUi::default(),
            pending: Vec::new(),
            last_consent: ConsentOutcome::Open,
        }
    }

    /// True only when a [`egui::TextEdit`] (or IME text capture) should steal keys.
    pub fn text_input_owns_keyboard(&self) -> bool {
        self.egui_ctx.text_edit_focused()
    }

    pub fn take_consent_outcome(&mut self) -> ConsentOutcome {
        std::mem::replace(&mut self.last_consent, ConsentOutcome::Open)
    }

    /// Close the palette editor on Escape (Configure Controls is a native window).
    pub fn dismiss_config_on_escape(&mut self) -> bool {
        if self.runtime.show_palette_editor {
            self.runtime.show_palette_editor = false;
            return true;
        }
        false
    }

    pub fn handle_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.egui_state.on_window_event(window, event).consumed
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    pub fn take_actions(&mut self) -> Vec<UiAction> {
        std::mem::take(&mut self.pending)
    }

    /// Prepare egui. `game_rgba` is the post-palette/effects frame (or `None` when empty).
    /// `fb_w`/`fb_h` are the logical framebuffer size (160×144 or 240×160).
    pub fn prepare(
        &mut self,
        window: &Window,
        settings: &mut FrontendSettings,
        game_rgba: Option<&[u8]>,
        fb_w: usize,
        fb_h: usize,
    ) {
        self.sync_game_texture(game_rgba, settings.display_mode, fb_w, fb_h);

        let raw_input = self.egui_state.take_egui_input(window);
        let mut actions = Vec::new();
        let mut runtime = std::mem::take(&mut self.runtime);
        let mut report = std::mem::take(&mut self.report);
        let mut consent = ConsentOutcome::Open;
        let rom_loaded = runtime.rom_loaded;
        let integer_scaling = settings.integer_scaling;
        let display_mode = settings.display_mode;
        let game_tex = self.game_texture.clone();

        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            egui::Panel::top("menu_bar")
                .resizable(false)
                .show_inside(ui, |ui| {
                    menu_bar(ui, settings, &mut runtime, &mut actions);
                });

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
                .show_inside(ui, |ui| {
                    if let Some(tex) = game_tex.as_ref() {
                        let avail = ui.available_size();
                        let (w, h) = game_image_size(
                            avail.x,
                            avail.y,
                            integer_scaling,
                            display_mode,
                            fb_w as f32,
                            fb_h as f32,
                        );
                        if w > 0.0 && h > 0.0 {
                            ui.centered_and_justified(|ui| {
                                ui.add(
                                    egui::Image::new(tex)
                                        .fit_to_exact_size(egui::vec2(w, h))
                                        .maintain_aspect_ratio(false),
                                );
                            });
                        }
                    } else if !rom_loaded {
                        empty_rom_screen(ui, settings, &mut actions);
                    }
                    if runtime.show_palette_editor {
                        let mut open = runtime.show_palette_editor;
                        palette_editor_window(ui.ctx(), settings, &mut open);
                        runtime.show_palette_editor = open;
                    }
                    playback_overlay(ui.ctx(), &runtime);
                });

            consent = show_report_ui(ui.ctx(), settings, &mut report);
        });
        self.runtime = runtime;
        self.report = report;
        self.last_consent = consent;
        self.pending.append(&mut actions);
        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(window, output.platform_output);
        self.paint_jobs = self
            .egui_ctx
            .tessellate(output.shapes, self.screen_descriptor.pixels_per_point);
    }

    fn sync_game_texture(
        &mut self,
        game_rgba: Option<&[u8]>,
        mode: DisplayMode,
        fb_w: usize,
        fb_h: usize,
    ) {
        let Some(rgba) = game_rgba else {
            self.game_texture = None;
            return;
        };
        let expected = fb_w.saturating_mul(fb_h).saturating_mul(4);
        if expected == 0 || rgba.len() < expected {
            self.game_texture = None;
            return;
        }
        let filter = if mode.prefers_linear_filter() {
            TextureOptions::LINEAR
        } else {
            TextureOptions::NEAREST
        };
        let image = ColorImage::from_rgba_unmultiplied([fb_w, fb_h], &rgba[..expected]);
        match self.game_texture.as_mut() {
            Some(tex) => {
                tex.set(image, filter);
            }
            None => {
                self.game_texture =
                    Some(self.egui_ctx.load_texture("gba_framebuffer", image, filter));
            }
        }
    }

    pub fn render(
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

/// Low-contrast playback HUD (after game texture; excluded from F6 screenshots).
fn playback_overlay(ctx: &Context, runtime: &RuntimeUi) {
    let (label, show) = if runtime.overlay_rewinding {
        ("REWIND", true)
    } else if runtime.overlay_frame_advance {
        ("FRAME +1", true)
    } else if runtime.overlay_paused {
        ("PAUSED", true)
    } else if let Some(speed) = runtime.overlay_speed.filter(|s| *s != SpeedPreset::X1) {
        (speed.label(), true)
    } else {
        ("", false)
    };
    if !show {
        return;
    }

    egui::Area::new(egui::Id::new("playback_overlay"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 36.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 140)),
            );
        });
}
