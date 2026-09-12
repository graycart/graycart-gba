//! Windowed host — eframe (winit/wgpu/egui) + cpal + core play loop.
//!
//! Cited: graycart-gba AGENTS.md (core ≠ GUI; FB + PCM + buttons only)
//!   Project store: `docs/graycart-gba/AGENTS.md`
//! Cited: graycart-gb `src/frontend/app.rs` posture (lean subset)
//!   https://github.com/graycart/graycart-gb/blob/main/src/frontend/app.rs
//! Note: playable GBA homebrew path; no DMG/CGB claim.

use super::audio::AudioOut;
use super::input::{buttons_from_keys, key_to_command, HostCommand};
use super::pace::FramePacer;
use super::rom::{is_rom_path, open_rom_dialog};
use super::sav_fs::{default_save_path, flush_save, load_save};
use super::ui::{self, UiAction};
use super::video::{rgb888_to_rgba_vec, SCALE, SCREEN_HEIGHT, SCREEN_WIDTH};
use eframe::egui::{self, ColorImage, Key, TextureHandle, TextureOptions};
use graycart_gba::Gba;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Open the application window. Flushes `.sav` on exit when a ROM was loaded.
pub fn run(initial_rom: Option<PathBuf>) -> Result<(), String> {
    let title = format!("graycart-gba {}", env!("CARGO_PKG_VERSION"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([
                (SCREEN_WIDTH as f32) * (SCALE as f32),
                (SCREEN_HEIGHT as f32) * (SCALE as f32) + 40.0,
            ]),
        ..Default::default()
    };
    eframe::run_native(
        "graycart-gba",
        options,
        Box::new(move |cc| Ok(Box::new(GbaApp::new(cc, initial_rom)))),
    )
    .map_err(|e| e.to_string())
}

struct GbaApp {
    gba: Option<Gba>,
    rom_path: Option<PathBuf>,
    paused: bool,
    keys_down: HashSet<Key>,
    audio: Option<AudioOut>,
    audio_error: Option<String>,
    game_tex: Option<TextureHandle>,
    pacer: FramePacer,
    status: String,
    pcm_scratch: Vec<graycart_gba::apu::PcmFrame>,
}

impl GbaApp {
    fn new(cc: &eframe::CreationContext<'_>, initial_rom: Option<PathBuf>) -> Self {
        let audio = match AudioOut::open_default() {
            Ok(a) => {
                let _ = a.sample_rate;
                Some(a)
            }
            Err(e) => {
                eprintln!("audio init soft-fail: {e}");
                None
            }
        };
        let audio_error = if audio.is_none() {
            Some("audio unavailable".into())
        } else {
            None
        };
        let mut app = Self {
            gba: None,
            rom_path: None,
            paused: false,
            keys_down: HashSet::new(),
            audio,
            audio_error,
            game_tex: None,
            pacer: FramePacer::new(true),
            status: "Open a .gba ROM to play.".into(),
            pcm_scratch: vec![graycart_gba::apu::PcmFrame::default(); 4096],
        };
        let _ = cc; // fonts / style left default for P9
        if let Some(path) = initial_rom {
            app.load_rom_path(&path);
        }
        app
    }

    fn load_rom_path(&mut self, path: &Path) {
        if !is_rom_path(path) {
            self.status = format!("not a .gba ROM: {}", path.display());
            return;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("failed to read {}: {e}", path.display());
                return;
            }
        };
        // Flush previous cart save first.
        self.flush_battery();
        let mut gba = Gba::new();
        gba.load_rom(&bytes);
        gba.reset_bios_hle();
        let sav_path = default_save_path(path);
        if let Ok(Some(sav)) = load_save(&sav_path) {
            gba.load_battery_sav(&sav);
            self.status = format!("loaded {} (+ {})", path.display(), sav_path.display());
        } else {
            self.status = format!("loaded {}", path.display());
        }
        self.gba = Some(gba);
        self.rom_path = Some(path.to_path_buf());
        self.paused = false;
    }

    fn flush_battery(&mut self) {
        let (Some(gba), Some(path)) = (self.gba.as_ref(), self.rom_path.as_ref()) else {
            return;
        };
        let sav = gba.battery_sav();
        if sav.is_empty() {
            return;
        }
        let sav_path = default_save_path(path);
        if let Err(e) = flush_save(&sav_path, &sav) {
            eprintln!("flush save failed: {e}");
            self.status = format!("save flush failed: {e}");
        }
    }

    fn reset_machine(&mut self) {
        if let Some(gba) = self.gba.as_mut() {
            gba.reset_bios_hle();
            self.status = "reset".into();
            self.paused = false;
        }
    }

    fn apply_ui_actions(&mut self, actions: &[UiAction], ctx: &egui::Context) {
        for a in actions {
            match a {
                UiAction::OpenRomDialog => {
                    if let Some(path) = open_rom_dialog() {
                        self.load_rom_path(&path);
                    }
                }
                UiAction::TogglePause => {
                    if self.gba.is_some() {
                        self.paused = !self.paused;
                        self.status = if self.paused {
                            "paused".into()
                        } else {
                            "running".into()
                        };
                    }
                }
                UiAction::Reset => self.reset_machine(),
                UiAction::Quit => {
                    self.flush_battery();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn handle_host_keys(&mut self, ctx: &egui::Context) {
        let mut cmds = Vec::new();
        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } = ev
                {
                    if let Some(cmd) = key_to_command(*key) {
                        cmds.push(cmd);
                    }
                }
            }
        });
        for cmd in cmds {
            match cmd {
                HostCommand::TogglePause => {
                    if self.gba.is_some() {
                        self.paused = !self.paused;
                    }
                }
                HostCommand::Reset => self.reset_machine(),
                HostCommand::OpenRom => {
                    if let Some(path) = open_rom_dialog() {
                        self.load_rom_path(&path);
                    }
                }
                HostCommand::Quit => {
                    self.flush_battery();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn tick_emulation(&mut self) {
        let Some(gba) = self.gba.as_mut() else {
            return;
        };
        if self.paused {
            return;
        }
        let mask = buttons_from_keys(self.keys_down.iter());
        gba.set_buttons(mask);
        gba.run_frames(1);
        if let Some(audio) = self.audio.as_ref() {
            let n = gba.pull_audio(&mut self.pcm_scratch);
            if n > 0 {
                audio.push_frames(&self.pcm_scratch[..n]);
            }
        }
        self.pacer.after_present();
    }

    fn update_game_texture(&mut self, ctx: &egui::Context) {
        let Some(gba) = self.gba.as_ref() else {
            return;
        };
        let rgb = gba.framebuffer_rgb();
        let rgba = rgb888_to_rgba_vec(&rgb);
        let image = ColorImage::from_rgba_unmultiplied([SCREEN_WIDTH, SCREEN_HEIGHT], &rgba);
        match self.game_tex.as_mut() {
            Some(tex) => {
                tex.set(image, TextureOptions::NEAREST);
            }
            None => {
                self.game_tex = Some(ctx.load_texture("gba_fb", image, TextureOptions::NEAREST));
            }
        }
    }
}

impl eframe::App for GbaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            self.keys_down.clear();
            for key in i.keys_down.iter() {
                self.keys_down.insert(*key);
            }
        });
        self.handle_host_keys(ctx);
        self.tick_emulation();
        self.update_game_texture(ctx);
        if self.gba.is_some() && !self.paused {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let menu_actions = ui::menu_bar(ui, self.gba.is_some(), self.paused);
        self.apply_ui_actions(&menu_actions, &ctx);

        if self.gba.is_none() {
            let mut actions = Vec::new();
            ui::empty_rom_screen(ui, &mut actions);
            self.apply_ui_actions(&actions, &ctx);
        } else if let Some(tex) = self.game_tex.as_ref() {
            let size = egui::vec2(
                (SCREEN_WIDTH as f32) * (SCALE as f32),
                (SCREEN_HEIGHT as f32) * (SCALE as f32),
            );
            ui.centered_and_justified(|ui| {
                ui.add(egui::Image::new(tex).fit_to_exact_size(size));
            });
        }
        ui.horizontal(|ui| {
            ui.label(&self.status);
            if self.paused {
                ui.label("[paused]");
            }
            if let Some(err) = &self.audio_error {
                ui.weak(err);
            }
        });
    }

    fn on_exit(&mut self) {
        self.flush_battery();
    }
}
