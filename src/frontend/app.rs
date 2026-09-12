//! Windowed host — eframe (winit/wgpu/egui) + cpal + core play loop.
//!
//! Cited: graycart-gba AGENTS.md (core ≠ GUI; FB + PCM + buttons only)
//!   Project store: `docs/graycart-gba/AGENTS.md`
//! Cited: graycart-gb `src/frontend/app.rs` posture (lean subset)
//!   https://github.com/graycart/graycart-gb/blob/main/src/frontend/app.rs
//! Note: P11 — dual session: native `.gba` or compat `.gb`/`.gbc`.

use super::audio::AudioOut;
use super::input::{buttons_from_keys, key_to_command, HostCommand};
use super::pace::FramePacer;
use super::rom::{is_rom_path, open_rom_dialog, rom_kind, RomKind};
use super::sav_fs::{default_save_path, flush_save, load_save};
use super::ui::{self, UiAction};
use super::video::{
    rgb888_to_rgba_vec, GB_SCREEN_HEIGHT, GB_SCREEN_WIDTH, SCALE, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use eframe::egui::{self, ColorImage, Key, TextureHandle, TextureOptions};
use graycart_gba::compat::{
    apply_lr_stretch, framebuffer_rgb888, gba_mask_to_gb_buttons, CompatMachine, StretchMode,
};
use graycart_gba::debug::DebugConfig;
use graycart_gba::Gba;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

enum HostSession {
    Native(Box<Gba>),
    Compat(Box<CompatMachine>),
}

/// Open the application window. Flushes `.sav` on exit when a ROM was loaded.
pub fn run(initial_rom: Option<PathBuf>, debug_cfg: DebugConfig) -> Result<(), String> {
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
        Box::new(move |cc| Ok(Box::new(GbaApp::new(cc, initial_rom, debug_cfg)))),
    )
    .map_err(|e| e.to_string())
}

struct GbaApp {
    session: Option<HostSession>,
    rom_path: Option<PathBuf>,
    paused: bool,
    keys_down: HashSet<Key>,
    audio: Option<AudioOut>,
    audio_error: Option<String>,
    game_tex: Option<TextureHandle>,
    pacer: FramePacer,
    status: String,
    pcm_scratch: Vec<graycart_gba::apu::PcmFrame>,
    stretch: StretchMode,
    lr_was_down: bool,
    debug_cfg: DebugConfig,
}

impl GbaApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        initial_rom: Option<PathBuf>,
        debug_cfg: DebugConfig,
    ) -> Self {
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
            session: None,
            rom_path: None,
            paused: false,
            keys_down: HashSet::new(),
            audio,
            audio_error,
            game_tex: None,
            pacer: FramePacer::new(true),
            status: "Open a .gba / .gb / .gbc ROM to play.".into(),
            pcm_scratch: vec![graycart_gba::apu::PcmFrame::default(); 4096],
            stretch: StretchMode::default(),
            lr_was_down: false,
            debug_cfg,
        };
        let _ = cc;
        if let Some(path) = initial_rom {
            app.load_rom_path(&path);
        }
        app
    }

    fn rom_loaded(&self) -> bool {
        self.session.is_some()
    }

    fn load_rom_path(&mut self, path: &Path) {
        let Some(kind) = rom_kind(path) else {
            self.status = format!("not a supported ROM: {}", path.display());
            return;
        };
        if !is_rom_path(path) {
            self.status = format!("not a supported ROM: {}", path.display());
            return;
        }
        self.flush_battery();
        self.game_tex = None;
        match kind {
            RomKind::Gba => self.load_gba(path),
            RomKind::GbCompat => self.load_compat(path),
        }
    }

    fn load_gba(&mut self, path: &Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("failed to read {}: {e}", path.display());
                return;
            }
        };
        let mut gba = Gba::new();
        gba.set_debug_config(self.debug_cfg);
        gba.load_rom(&bytes);
        gba.reset_bios_hle();
        let sav_path = default_save_path(path);
        if let Ok(Some(sav)) = load_save(&sav_path) {
            gba.load_battery_sav(&sav);
            self.status = format!("loaded {} (+ {})", path.display(), sav_path.display());
        } else {
            self.status = format!("loaded {}", path.display());
        }
        if self.debug_cfg.enabled() {
            eprintln!(
                "gba-debug: host loaded {} (BiosHle); breadcrumbs on stderr",
                path.display()
            );
        }
        self.session = Some(HostSession::Native(Box::new(gba)));
        self.rom_path = Some(path.to_path_buf());
        self.paused = false;
    }

    fn load_compat(&mut self, path: &Path) {
        let mut machine = CompatMachine::new();
        if let Err(e) = machine.load_rom_path(path) {
            self.status = format!("compat load failed: {e}");
            return;
        }
        let sav_path = default_save_path(path);
        if let Ok(Some(sav)) = load_save(&sav_path) {
            machine.load_battery_image(&sav);
            self.status = format!(
                "loaded compat {} (+ {}) [{}]",
                path.display(),
                sav_path.display(),
                match machine.silicon() {
                    graycart_gba::compat::CompatSilicon::FastDmg => "FastDmg",
                    graycart_gba::compat::CompatSilicon::FastCgb => "FastCgb",
                }
            );
        } else {
            self.status = format!(
                "loaded compat {} [{}]",
                path.display(),
                match machine.silicon() {
                    graycart_gba::compat::CompatSilicon::FastDmg => "FastDmg",
                    graycart_gba::compat::CompatSilicon::FastCgb => "FastCgb",
                }
            );
        }
        self.session = Some(HostSession::Compat(Box::new(machine)));
        self.rom_path = Some(path.to_path_buf());
        self.paused = false;
        self.stretch = StretchMode::default();
        self.lr_was_down = false;
    }

    fn flush_battery(&mut self) {
        let Some(path) = self.rom_path.as_ref() else {
            return;
        };
        let sav = match self.session.as_ref() {
            Some(HostSession::Native(gba)) => {
                let bytes = gba.battery_sav();
                if bytes.is_empty() {
                    return;
                }
                bytes
            }
            Some(HostSession::Compat(m)) => match m.battery_image() {
                Some(b) if !b.is_empty() => b,
                _ => return,
            },
            None => return,
        };
        let sav_path = default_save_path(path);
        if let Err(e) = flush_save(&sav_path, &sav) {
            eprintln!("flush save failed: {e}");
            self.status = format!("save flush failed: {e}");
        }
    }

    fn reset_machine(&mut self) {
        match self.session.as_mut() {
            Some(HostSession::Native(gba)) => {
                gba.reset_bios_hle();
                self.status = "reset".into();
                self.paused = false;
            }
            Some(HostSession::Compat(m)) => match m.reset() {
                Ok(()) => {
                    self.status = "reset".into();
                    self.paused = false;
                }
                Err(e) => self.status = format!("reset failed: {e}"),
            },
            None => {}
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
                    if self.rom_loaded() {
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
                    if self.rom_loaded() {
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
        if self.paused {
            return;
        }
        let mask = buttons_from_keys(self.keys_down.iter());
        match self.session.as_mut() {
            Some(HostSession::Native(gba)) => {
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
            Some(HostSession::Compat(machine)) => {
                let (btns, lr_now) = gba_mask_to_gb_buttons(mask);
                let lr_edge = lr_now && !self.lr_was_down;
                self.lr_was_down = lr_now;
                if lr_edge {
                    self.stretch = apply_lr_stretch(self.stretch, true);
                }
                machine.set_buttons(&btns);
                let _ = machine.run_frames(1);
                if let Some(audio) = self.audio.as_ref() {
                    let samples = machine.drain_audio();
                    if !samples.is_empty() {
                        let pairs: Vec<(f32, f32)> =
                            samples.iter().map(|s| (s.left, s.right)).collect();
                        audio.push_stereo_f32(&pairs);
                    }
                }
                self.pacer.after_present();
            }
            None => {}
        }
    }

    fn update_game_texture(&mut self, ctx: &egui::Context) {
        match self.session.as_ref() {
            Some(HostSession::Native(gba)) => {
                let rgb = gba.framebuffer_rgb();
                let rgba = rgb888_to_rgba_vec(&rgb, SCREEN_WIDTH, SCREEN_HEIGHT);
                let image =
                    ColorImage::from_rgba_unmultiplied([SCREEN_WIDTH, SCREEN_HEIGHT], &rgba);
                match self.game_tex.as_mut() {
                    Some(tex) => tex.set(image, TextureOptions::NEAREST),
                    None => {
                        self.game_tex =
                            Some(ctx.load_texture("gba_fb", image, TextureOptions::NEAREST));
                    }
                }
            }
            Some(HostSession::Compat(machine)) => {
                let shades = machine.framebuffer_shades();
                let rgb = framebuffer_rgb888(shades);
                let rgba = rgb888_to_rgba_vec(&rgb, GB_SCREEN_WIDTH, GB_SCREEN_HEIGHT);
                let image =
                    ColorImage::from_rgba_unmultiplied([GB_SCREEN_WIDTH, GB_SCREEN_HEIGHT], &rgba);
                match self.game_tex.as_mut() {
                    Some(tex) => tex.set(image, TextureOptions::NEAREST),
                    None => {
                        self.game_tex =
                            Some(ctx.load_texture("gb_fb", image, TextureOptions::NEAREST));
                    }
                }
            }
            None => {}
        }
    }

    fn present_size(&self) -> egui::Vec2 {
        match self.session.as_ref() {
            Some(HostSession::Compat(_)) => {
                let w = self.stretch.present_width();
                egui::vec2(
                    (w as f32) * (SCALE as f32),
                    (GB_SCREEN_HEIGHT as f32) * (SCALE as f32),
                )
            }
            _ => egui::vec2(
                (SCREEN_WIDTH as f32) * (SCALE as f32),
                (SCREEN_HEIGHT as f32) * (SCALE as f32),
            ),
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
        if self.rom_loaded() && !self.paused {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let menu_actions = ui::menu_bar(ui, self.rom_loaded(), self.paused);
        self.apply_ui_actions(&menu_actions, &ctx);

        if !self.rom_loaded() {
            let mut actions = Vec::new();
            ui::empty_rom_screen(ui, &mut actions);
            self.apply_ui_actions(&actions, &ctx);
        } else if let Some(tex) = self.game_tex.as_ref() {
            let size = self.present_size();
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
