//! Misc system regs: HALTCNT Halt/Stop, POSTFLG, WAITCNT presence, SIO stubs (P3).
//!
//! Cited: GBATEK — GBA System Control (POSTFLG / HALTCNT / WAITCNT)
//!   https://problemkaputt.de/gbatek-gba-system-control.htm
//! Cited: GBATEK — BIOS Halt / Stop / IntrWait
//!   https://problemkaputt.de/gbatek.htm#bioshaltfunctions
//! Cited: GBATEK — Serial I/O Overview (register presence only)
//!   https://problemkaputt.de/gbatek-gbacommunicationports.htm
//! Cross-check: research `docs/graycart-gba/05-io-timers-irq-input.md` §§5,7, WAITCNT §2.4.
//! Note: Stop fidelity / full SIO protocols deferred (TBD). WAITCNT wait *tables*
//! live in `crate::bus::waitcnt` — this module only holds MMIO presence / raw R/W.

use crate::bus::waitcnt::WaitCnt;

/// `HALTCNT` (`04000301`) — write-only; bit7 selects Halt vs Stop.
pub const HALTCNT_ADDR: u32 = 0x0400_0301;
/// `POSTFLG` (`04000300`).
pub const POSTFLG_ADDR: u32 = 0x0400_0300;
/// `WAITCNT` (`04000204`).
pub const WAITCNT_ADDR: u32 = 0x0400_0204;

/// `SIODATA32` / low half of multiplayer data (`04000120`).
pub const SIO_DATA32_LO_ADDR: u32 = 0x0400_0120;
/// `SIOCNT` (`04000128`).
pub const SIOCNT_ADDR: u32 = 0x0400_0128;
/// `RCNT` (`04000134`).
pub const RCNT_ADDR: u32 = 0x0400_0134;

/// CPU power / sleep mode after a HALTCNT write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerMode {
    /// Running normally.
    #[default]
    Run,
    /// Halt: CPU paused; timers/video/etc. keep running. Wake on `(IE & IF) != 0`.
    Halt,
    /// Stop: very low power — **stub**. Wake sources TBD; not modeled deeply in P3.
    Stop,
}

/// System hardware block owned by the P3 hw/halt stream.
#[derive(Debug, Clone)]
pub struct Hw {
    /// Current CPU sleep mode.
    pub power: PowerMode,
    /// `POSTFLG` low byte (bit0 first-boot flag).
    pub postflg: u8,
    /// WAITCNT presence — wraps bus waitcnt type (tables stay in `bus::waitcnt`).
    pub waitcnt: WaitCnt,
    /// SIO / RCNT register file stubs (presence + R/W only).
    pub sio_data32: u32,
    pub siocnt: u16,
    pub rcnt: u16,
}

impl Default for Hw {
    fn default() -> Self {
        Self {
            power: PowerMode::Run,
            postflg: 0,
            waitcnt: WaitCnt::power_on(),
            sio_data32: 0,
            siocnt: 0,
            rcnt: 0,
        }
    }
}

impl Hw {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True while the CPU should not retire instructions (Halt or Stop).
    #[inline]
    pub fn cpu_sleeping(&self) -> bool {
        !matches!(self.power, PowerMode::Run)
    }

    /// Write `HALTCNT` (`04000301`). Bit7=0 → Halt, bit7=1 → Stop.
    ///
    /// BIOS typically writes `0x00` (Halt) or `0x80` (Stop).
    pub fn write_haltcnt(&mut self, value: u8) {
        self.power = if value & 0x80 != 0 {
            PowerMode::Stop
        } else {
            PowerMode::Halt
        };
    }

    /// Halt wake check: `(IE & IF) != 0` — **IME and CPSR.I are don't-care** (GBATEK).
    ///
    /// `ie_and_if` should be the bitwise AND of the IE and IF halfwords (or any
    /// nonzero pending-enabled mask). Callers (irq stream / Gba step) supply it.
    pub fn poll_halt_wake(&mut self, ie_and_if: u16) {
        if matches!(self.power, PowerMode::Halt) && ie_and_if != 0 {
            self.power = PowerMode::Run;
        }
        // Stop wake is keypad / Game Pak / GP-SIO only — not implemented (TBD).
    }

    /// Enter Halt explicitly (SWI Halt / CustomHalt `00h` path).
    pub fn enter_halt(&mut self) {
        self.power = PowerMode::Halt;
    }

    // --- POSTFLG ---

    #[inline]
    pub fn read_postflg(&self) -> u8 {
        self.postflg
    }

    #[inline]
    pub fn write_postflg(&mut self, value: u8) {
        self.postflg = value;
    }

    // --- WAITCNT presence (delegates to bus::WaitCnt) ---

    #[inline]
    pub fn read_waitcnt(&self) -> u16 {
        self.waitcnt.raw()
    }

    #[inline]
    pub fn write_waitcnt(&mut self, value: u16) {
        self.waitcnt.write(value);
    }

    // --- SIO stubs ---

    #[inline]
    pub fn read_sio_data32(&self) -> u32 {
        self.sio_data32
    }

    #[inline]
    pub fn write_sio_data32(&mut self, value: u32) {
        self.sio_data32 = value;
    }

    #[inline]
    pub fn read_siocnt(&self) -> u16 {
        self.siocnt
    }

    #[inline]
    pub fn write_siocnt(&mut self, value: u16) {
        self.siocnt = value;
    }

    #[inline]
    pub fn read_rcnt(&self) -> u16 {
        self.rcnt
    }

    #[inline]
    pub fn write_rcnt(&mut self, value: u16) {
        self.rcnt = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::waitcnt::{COMMERCIAL_COMMON, POWER_ON};

    #[test]
    fn haltcnt_bit7_selects_halt_vs_stop() {
        let mut hw = Hw::new();
        hw.write_haltcnt(0x00);
        assert_eq!(hw.power, PowerMode::Halt);
        assert!(hw.cpu_sleeping());

        hw.power = PowerMode::Run;
        hw.write_haltcnt(0x80);
        assert_eq!(hw.power, PowerMode::Stop);
        assert!(hw.cpu_sleeping());
    }

    #[test]
    fn halt_wakes_on_ie_and_if_nonzero_ime_dont_care() {
        let mut hw = Hw::new();
        hw.enter_halt();
        // IME=0 path: still wake when any IE∧IF bit is set.
        hw.poll_halt_wake(0);
        assert_eq!(hw.power, PowerMode::Halt);
        hw.poll_halt_wake(1 << 3); // timer0 pending+enabled
        assert_eq!(hw.power, PowerMode::Run);
    }

    #[test]
    fn stop_does_not_wake_on_timer_ie_if_stub() {
        let mut hw = Hw::new();
        hw.write_haltcnt(0x80);
        hw.poll_halt_wake(0xFFFF);
        assert_eq!(
            hw.power,
            PowerMode::Stop,
            "Stop wake is keypad/cart/GP-SIO only — timer IE∧IF must not clear Stop yet"
        );
    }

    #[test]
    fn postflg_rw_presence() {
        let mut hw = Hw::new();
        assert_eq!(hw.read_postflg(), 0);
        hw.write_postflg(0x01);
        assert_eq!(hw.read_postflg(), 0x01);
        assert_eq!(POSTFLG_ADDR, 0x0400_0300);
        assert_eq!(HALTCNT_ADDR, 0x0400_0301);
    }

    #[test]
    fn waitcnt_presence_delegates_to_bus_type() {
        let mut hw = Hw::new();
        assert_eq!(hw.read_waitcnt(), POWER_ON);
        hw.write_waitcnt(COMMERCIAL_COMMON);
        assert_eq!(hw.read_waitcnt(), COMMERCIAL_COMMON & 0x5FFF); // writable mask
        assert_eq!(WAITCNT_ADDR, 0x0400_0204);
    }

    #[test]
    fn sio_regs_rw_presence_stubs() {
        let mut hw = Hw::new();
        hw.write_sio_data32(0xDEAD_BEEF);
        assert_eq!(hw.read_sio_data32(), 0xDEAD_BEEF);
        hw.write_siocnt(0x4003);
        assert_eq!(hw.read_siocnt(), 0x4003);
        hw.write_rcnt(0x8000);
        assert_eq!(hw.read_rcnt(), 0x8000);
        assert_eq!(SIO_DATA32_LO_ADDR, 0x0400_0120);
        assert_eq!(SIOCNT_ADDR, 0x0400_0128);
        assert_eq!(RCNT_ADDR, 0x0400_0134);
    }
}
