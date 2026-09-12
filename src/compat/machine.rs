//! graycart Cpu+Bus wrapper — P10 **G10-wrap** / **G10-api**.
//!
//! Cited: graycart-gba `10-core-api-and-gb-reuse.md` §4
//!   Project store: `docs/graycart-gba/10-core-api-and-gb-reuse.md`
//! Cited: graycart-gb public machine surface (`Cpu`, `Bus`, `apply_fast`, …)
//!   https://github.com/graycart/graycart-gb
//! Note: whole-crate dep interim; no in-tree SM83. FastHle uses `apply_fast`.

use super::boot::{require_boot, AgbBootFirmware, CompatBootMode, Mode8Handoff};
use super::detect::{hint_from_extension, LoadPathHint, MachineProfile};
use graycart::{
    apply_fast, step, Bus, Cartridge, Cpu, ExecSession, GameBoyButton, RunOutcome, Shade,
    StereoSample, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use std::path::Path;

/// Headless DMG/CGB machine owned by the GBA compat wrapper.
pub struct CompatMachine {
    cpu: Cpu,
    bus: Bus,
    session: ExecSession,
    boot_mode: CompatBootMode,
    firmware: AgbBootFirmware,
    handoff: Mode8Handoff,
    /// Frames retired via [`Self::run_frames`].
    pub frames: u64,
}

impl Default for CompatMachine {
    fn default() -> Self {
        let cart = Cartridge::rom_only(vec![0; 0x8000]);
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(cart);
        apply_fast(&mut cpu, &mut bus);
        Self {
            cpu,
            bus,
            session: ExecSession::new(),
            boot_mode: CompatBootMode::FastHle,
            firmware: AgbBootFirmware::empty(),
            handoff: Mode8Handoff::native(),
            frames: 0,
        }
    }
}

impl CompatMachine {
    /// Create an empty placeholder (FastHle, no real cart).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Family-ish system id for host seams.
    #[must_use]
    pub const fn system_id(&self) -> &'static str {
        "gb-compat"
    }

    /// Current profile (compat path).
    #[must_use]
    pub const fn profile(&self) -> MachineProfile {
        MachineProfile::CompatGb
    }

    /// Mode-8 / HALT latch after entry.
    #[must_use]
    pub const fn handoff(&self) -> Mode8Handoff {
        self.handoff
    }

    /// Install CGB-AGB boot ROM bytes (user-supplied).
    pub fn load_agb_boot_rom(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.firmware.load(bytes)
    }

    /// Select FastHle vs BootRomLle (LLE needs firmware).
    pub fn set_boot_mode(&mut self, mode: CompatBootMode) -> Result<(), String> {
        require_boot(mode, &self.firmware)?;
        self.boot_mode = mode;
        Ok(())
    }

    /// Load `.gb` / `.gbc` from a filesystem path (DMG silicon + FastHle).
    pub fn load_rom_path(&mut self, path: &Path) -> Result<(), String> {
        let hint = hint_from_extension(path.extension().and_then(|e| e.to_str()));
        if hint == LoadPathHint::GbaRom {
            return Err("CompatMachine expects .gb/.gbc — use Gba for .gba".into());
        }
        let cart = Cartridge::load(path).map_err(|e| e.to_string())?;
        self.install_cart(cart)
    }

    /// Load ROM bytes via a temp file (graycart `Cartridge` is path-oriented).
    pub fn load_rom_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "graycart-gba-compat-{}-{}.gb",
            std::process::id(),
            self.frames
        ));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        let result = self.load_rom_path(&path);
        let _ = std::fs::remove_file(&path);
        result
    }

    fn install_cart(&mut self, cart: Cartridge) -> Result<(), String> {
        require_boot(self.boot_mode, &self.firmware)?;
        match self.boot_mode {
            CompatBootMode::FastHle => {
                let mut cpu = Cpu::new();
                let mut bus = Bus::new(cart);
                apply_fast(&mut cpu, &mut bus);
                self.cpu = cpu;
                self.bus = bus;
                self.session = ExecSession::new();
                self.frames = 0;
                self.handoff = Mode8Handoff::enter_compat();
                Ok(())
            }
            CompatBootMode::BootRomLle => {
                Err("BootRomLle SM83 overlay wiring is stretch — use FastHle for P10 smoke".into())
            }
        }
    }

    /// Soft reset under the current boot mode (re-applies FastHle).
    pub fn reset(&mut self) -> Result<(), String> {
        require_boot(self.boot_mode, &self.firmware)?;
        match self.boot_mode {
            CompatBootMode::FastHle => {
                apply_fast(&mut self.cpu, &mut self.bus);
                self.session = ExecSession::new();
                self.frames = 0;
                self.handoff = Mode8Handoff::enter_compat();
                Ok(())
            }
            CompatBootMode::BootRomLle => {
                Err("BootRomLle reset stretch — use FastHle for P10".into())
            }
        }
    }

    /// Replace pressed Game Boy buttons (missing = released).
    pub fn set_buttons(&mut self, pressed: &[GameBoyButton]) {
        for b in GameBoyButton::ALL {
            self.bus.release_button(b);
        }
        for &b in pressed {
            self.bus.press_button(b);
        }
    }

    /// Run until `n` additional VBlank frames (graycart `ExecSession`).
    pub fn run_frames(&mut self, n: u64) -> Result<RunOutcome, String> {
        if n == 0 {
            return Ok(RunOutcome::FrameLimit {
                frames: self.session.frames,
                steps: self.session.steps,
            });
        }
        let target = self.session.frames.saturating_add(n);
        let outcome = self
            .session
            .run_frames(&mut self.cpu, &mut self.bus, target);
        self.frames = self.session.frames;
        Ok(outcome)
    }

    /// Step a single SM83 instruction (smoke / oracle helpers).
    pub fn step_instruction(&mut self) -> Result<(), String> {
        step(&mut self.cpu, &mut self.bus).map_err(|e| format!("{e:?}"))?;
        Ok(())
    }

    /// Borrow the 160×144 shade framebuffer.
    #[must_use]
    pub fn framebuffer_shades(&self) -> &[Shade] {
        self.bus.ppu.framebuffer.pixels()
    }

    /// Screen geometry (DMG/CGB panel).
    #[must_use]
    pub const fn screen_size() -> (usize, usize) {
        (SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    /// Drain host PCM samples generated since last drain.
    pub fn drain_audio(&mut self) -> Vec<StereoSample> {
        self.bus.apu.take_samples()
    }

    /// Battery save image when the cart has backup RAM.
    #[must_use]
    pub fn battery_image(&self) -> Option<Vec<u8>> {
        if !self.bus.cartridge.needs_save() {
            return None;
        }
        Some(self.bus.cartridge.external_ram().to_vec())
    }

    /// Load battery bytes into cart RAM when present.
    pub fn load_battery_image(&mut self, data: &[u8]) {
        if self.bus.cartridge.needs_save() {
            self.bus.cartridge.load_external_ram(data);
        }
    }

    /// Serial text accumulated by graycart (Blargg oracle).
    #[must_use]
    pub fn serial_text(&self) -> String {
        self.bus.serial_text()
    }

    /// Read bus byte (Blargg RAM oracle helpers).
    #[must_use]
    pub fn read8(&self, addr: u16) -> u8 {
        self.bus.read8(addr)
    }

    /// Borrow CPU for tests / oracles.
    #[must_use]
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_machine_is_compat_profile() {
        let m = CompatMachine::new();
        assert_eq!(m.profile(), MachineProfile::CompatGb);
        assert_eq!(m.system_id(), "gb-compat");
        assert_eq!(CompatMachine::screen_size(), (160, 144));
    }

    #[test]
    fn load_rom_only_bytes_and_run_one_frame() {
        let c = Cartridge::rom_only(vec![0x00; 0x8000]);
        let bytes = c.rom_bytes().to_vec();
        let mut m = CompatMachine::new();
        m.load_rom_bytes(&bytes).unwrap();
        assert!(m.handoff().sm83_active);
        let outcome = m.run_frames(1).unwrap();
        match outcome {
            RunOutcome::FrameLimit { frames, .. } => assert!(frames >= 1),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(m.framebuffer_shades().len(), 160 * 144);
        let _ = m.drain_audio();
    }

    #[test]
    fn rejects_gba_extension() {
        let mut m = CompatMachine::new();
        let path = std::env::temp_dir().join("not-really.gba");
        std::fs::write(&path, [0u8; 16]).unwrap();
        let err = m.load_rom_path(&path).unwrap_err();
        assert!(err.contains(".gb"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_buttons_does_not_panic() {
        let mut m = CompatMachine::new();
        m.set_buttons(&[GameBoyButton::A, GameBoyButton::Start]);
        m.set_buttons(&[]);
    }
}
