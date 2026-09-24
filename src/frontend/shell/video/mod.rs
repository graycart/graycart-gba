//! Host video: palette + effects + GPU present via `pixels`.

mod effects;
mod layout;
mod palette;
mod rgb555;

pub use effects::{DisplayMode, apply_effect};
pub use layout::game_image_size;
pub use palette::{Palette, PalettePreset, fill_rgba, framebuffer_to_rgba};
pub use rgb555::rgb555_framebuffer_to_rgba;

use graycart::{Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH};
use pixels::{Pixels, PixelsBuilder, PixelsContext, SurfaceTexture, wgpu};
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::window::{Fullscreen, Window};

/// Default integer scale factor for the play window.
pub const SCALE: u32 = 3;
pub const GBA_WIDTH: u32 = 240;
pub const GBA_HEIGHT: u32 = 160;

/// Windowed GPU presenter for the emulator framebuffer.
pub struct Renderer {
    window: Arc<Window>,
    pixels: Pixels<'static>,
    base_title: String,
    /// Soft-blur scratch / DMG ghosting history (reused; not allocated on Sharp).
    previous_frame: Option<Vec<u8>>,
}

impl Renderer {
    /// Create the presenter. Surface is sized for GBA (240×160); SM83 content uses a
    /// 160×144 region of the same buffer. `vsync` should stay **false** when
    /// [`FramePacer`] owns wall-clock cadence.
    pub fn new(window: Arc<Window>, title: &str, vsync: bool) -> Result<Self, String> {
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, Arc::clone(&window));
        let pixels = PixelsBuilder::new(GBA_WIDTH, GBA_HEIGHT, surface)
            .enable_vsync(vsync)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            window,
            pixels,
            base_title: title.to_string(),
            previous_frame: None,
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn pixels(&self) -> &Pixels<'static> {
        &self.pixels
    }

    pub fn set_fullscreen(&self, enabled: bool) {
        self.window.set_fullscreen(if enabled {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
    }

    pub fn write_framebuffer(&mut self, fb: &Framebuffer, palette: Palette, mode: DisplayMode) {
        present_framebuffer_rgba(
            fb,
            palette,
            mode,
            self.pixels.frame_mut(),
            &mut self.previous_frame,
        );
    }

    /// Direct GBA BGR555 → RGBA into the pixels buffer (no DMG palette).
    pub fn write_gba_bgr555(&mut self, pixels: &[u16]) {
        let frame = self.pixels.frame_mut();
        // Resize is not available on every pixels build; write into the front of the buffer.
        crate::frontend::gba_framebuffer_to_rgba(pixels, frame);
        self.previous_frame = None;
    }

    /// CPU RGBA after palette + effects — same bytes presented to the GPU scaler.
    #[allow(dead_code)] // Task 6 screenshot hotkey
    pub fn presented_rgba(&self) -> &[u8] {
        self.pixels.frame()
    }

    pub fn write_empty(&mut self, palette: Palette) {
        let rgba = palette.shade_to_rgba(graycart::Shade::Darkest);
        fill_rgba(self.pixels.frame_mut(), rgba);
        self.previous_frame = None;
    }

    /// Clear the surface, then run `overlay` (egui) — game pixels are drawn by egui
    /// into the central content rect below the menu bar (not via pixels scaling).
    pub fn present_with(
        &mut self,
        mut overlay: impl FnMut(
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            &PixelsContext<'_>,
        ) -> Result<(), pixels::Error>,
    ) -> Result<(), String> {
        self.pixels
            .render_with(|encoder, render_target, context| {
                // Clear letterbox / menu chrome; game image is an egui widget.
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("graycart_clear"),
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
                Ok(overlay(encoder, render_target, context)?)
            })
            .map_err(|e| e.to_string())
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.pixels
            .resize_surface(width, height)
            .map_err(|e| e.to_string())
    }

    pub fn set_base_title(&mut self, title: &str) {
        self.base_title = title.to_string();
        self.window.set_title(title);
    }

    pub fn set_fps_title(&self, fps: f64) {
        self.window
            .set_title(&format!("{} — {fps:.1} FPS", self.base_title));
    }
}

/// Default window attributes for a scaled 160×144 LCD plus menu bar chrome.
pub fn window_attributes(title: &str) -> winit::window::WindowAttributes {
    // ~28 logical px reserves room so the 4× game area sits fully below the menu.
    const MENU_CHROME: f64 = 28.0;
    let size = LogicalSize::new(
        (SCREEN_WIDTH as u32 * SCALE) as f64,
        (SCREEN_HEIGHT as u32 * SCALE) as f64 + MENU_CHROME,
    );
    Window::default_attributes()
        .with_title(title)
        .with_inner_size(size)
        .with_min_inner_size(LogicalSize::new(
            SCREEN_WIDTH as f64,
            SCREEN_HEIGHT as f64 + MENU_CHROME,
        ))
        .with_resizable(true)
}

/// Shade→palette for DMG/compat; RGB555→RGBA for NativeCgb. Then host effects.
pub fn present_framebuffer_rgba(
    fb: &Framebuffer,
    palette: Palette,
    mode: DisplayMode,
    out: &mut [u8],
    previous: &mut Option<Vec<u8>>,
) {
    if fb.presents_cgb_color() {
        rgb555_framebuffer_to_rgba(fb.rgb555_pixels(), out);
    } else {
        framebuffer_to_rgba(fb, palette, out);
    }
    apply_effect(mode, out, previous);
}
