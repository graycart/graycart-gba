//! ARM7TDMI register file: GPRs, CPSR, SPSRs, and banking.
//!
//! Cited: GBATEK -- ARM CPU Register Set
//!   https://problemkaputt.de/gbatek.htm
//! Cited: GBATEK -- ARM CPU Flags / Condition Field
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C (ARM7TDMI TRM r4p1) -- programmer's model
//! Note: 37×32-bit architectural registers with per-mode banking; CPSR T must
//! change via BX / exception entry·return, not casual MSR. Reserved CPSR bits
//! are preserved (no ARMv5 Q/J inventing on GBA).

use super::mode::Mode;

/// CPSR / SPSR bit masks (ARMv4T / GBA-relevant).
pub mod cpsr {
    pub const N: u32 = 1 << 31;
    pub const Z: u32 = 1 << 30;
    pub const C: u32 = 1 << 29;
    pub const V: u32 = 1 << 28;
    /// IRQ disable.
    pub const I: u32 = 1 << 7;
    /// FIQ disable.
    pub const F: u32 = 1 << 6;
    /// Thumb state (0 = ARM, 1 = Thumb).
    pub const T: u32 = 1 << 5;
    pub const MODE_MASK: u32 = 0x1F;

    /// NZCV flags field (MSR `_f` / bits 31:24 in PSR transfer).
    pub const FLAGS_MASK: u32 = 0xFF00_0000;
    /// Status field (MSR `_s` / bits 23:16) — reserved on ARMv4T; still maskable.
    pub const STATUS_MASK: u32 = 0x00FF_0000;
    /// Extension field (MSR `_x` / bits 15:8) — reserved on ARMv4T.
    pub const EXTENSION_MASK: u32 = 0x0000_FF00;
    /// Control field (MSR `_c` / bits 7:0): I/F/T/mode.
    pub const CONTROL_MASK: u32 = 0x0000_00FF;
}

/// Full ARM7TDMI register file with mode banking.
///
/// R0–R7 and R15 are unbanked. R8–R12 are banked only in FIQ. R13/R14 and SPSR
/// are banked per exception mode; User and System share the User bank.
#[derive(Debug, Clone)]
pub struct Regs {
    r0_r7: [u32; 8],
    r8_r12_usr: [u32; 5],
    r8_r12_fiq: [u32; 5],
    /// R13 per [`Bank`] (Usr, Fiq, Irq, Svc, Abt, Und).
    r13: [u32; 6],
    /// R14 per [`Bank`].
    r14: [u32; 6],
    r15: u32,
    cpsr: u32,
    /// SPSR_fiq, SPSR_irq, SPSR_svc, SPSR_abt, SPSR_und.
    spsr: [u32; 5],
}

impl Default for Regs {
    /// Power-on / reset-ish CPSR: Supervisor, ARM, IRQ+FIQ masked (GBATEK reset).
    fn default() -> Self {
        Self {
            r0_r7: [0; 8],
            r8_r12_usr: [0; 5],
            r8_r12_fiq: [0; 5],
            r13: [0; 6],
            r14: [0; 6],
            r15: 0,
            cpsr: Mode::Supervisor.bits() | cpsr::I | cpsr::F,
            spsr: [0; 5],
        }
    }
}

impl Regs {
    /// Construct with reset CPSR (same as [`Default`]).
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current processor mode from CPSR.
    #[inline]
    pub fn mode(&self) -> Mode {
        Mode::from_bits(self.cpsr & cpsr::MODE_MASK).unwrap_or(Mode::Supervisor)
    }

    /// Read GPR `r` (0–15). R15 is the architectural (fetch) PC with no extra skew.
    #[inline]
    pub fn get(&self, r: u8) -> u32 {
        debug_assert!(r < 16);
        match r {
            0..=7 => self.r0_r7[r as usize],
            8..=12 => {
                if self.mode() == Mode::Fiq {
                    self.r8_r12_fiq[(r - 8) as usize]
                } else {
                    self.r8_r12_usr[(r - 8) as usize]
                }
            }
            13 => self.r13[self.mode().bank().index()],
            14 => self.r14[self.mode().bank().index()],
            15 => self.r15,
            _ => unreachable!("register index out of range"),
        }
    }

    /// Write GPR `r` (0–15). Does **not** force-align R15; use [`Self::set_pc`] /
    /// [`Self::branch_exchange`] for architectural PC writes.
    #[inline]
    pub fn set(&mut self, r: u8, value: u32) {
        debug_assert!(r < 16);
        match r {
            0..=7 => self.r0_r7[r as usize] = value,
            8..=12 => {
                if self.mode() == Mode::Fiq {
                    self.r8_r12_fiq[(r - 8) as usize] = value;
                } else {
                    self.r8_r12_usr[(r - 8) as usize] = value;
                }
            }
            13 => self.r13[self.mode().bank().index()] = value,
            14 => self.r14[self.mode().bank().index()] = value,
            15 => self.r15 = value,
            _ => unreachable!("register index out of range"),
        }
    }

    /// Read a banked R13 without changing mode (for exception / debug paths).
    #[inline]
    pub fn get_r13_mode(&self, mode: Mode) -> u32 {
        self.r13[mode.bank().index()]
    }

    /// Write a banked R13 without changing mode.
    #[inline]
    pub fn set_r13_mode(&mut self, mode: Mode, value: u32) {
        self.r13[mode.bank().index()] = value;
    }

    /// Read a banked R14 without changing mode.
    #[inline]
    pub fn get_r14_mode(&self, mode: Mode) -> u32 {
        self.r14[mode.bank().index()]
    }

    /// Write a banked R14 without changing mode.
    #[inline]
    pub fn set_r14_mode(&mut self, mode: Mode, value: u32) {
        self.r14[mode.bank().index()] = value;
    }

    /// Read GPR from the **User** bank (LDM/STM `^` force-user transfers).
    ///
    /// R0–R7 and R15 are unbanked. R8–R12 use the User/System FIQ-shared bank.
    /// R13/R14 use the User bank. Cited: GBATEK block transfer S-bit.
    #[inline]
    pub fn get_user(&self, r: u8) -> u32 {
        debug_assert!(r < 16);
        match r {
            0..=7 => self.r0_r7[r as usize],
            8..=12 => self.r8_r12_usr[(r - 8) as usize],
            13 => self.r13[Mode::User.bank().index()],
            14 => self.r14[Mode::User.bank().index()],
            15 => self.r15,
            _ => unreachable!("register index out of range"),
        }
    }

    /// Write GPR in the **User** bank (LDM `^` without R15).
    #[inline]
    pub fn set_user(&mut self, r: u8, value: u32) {
        debug_assert!(r < 16);
        match r {
            0..=7 => self.r0_r7[r as usize] = value,
            8..=12 => self.r8_r12_usr[(r - 8) as usize] = value,
            13 => self.r13[Mode::User.bank().index()] = value,
            14 => self.r14[Mode::User.bank().index()] = value,
            15 => self.r15 = value,
            _ => unreachable!("register index out of range"),
        }
    }

    /// Architectural PC (R15). Pipeline skew is applied by the execute / pipeline layer.
    #[inline]
    pub fn pc(&self) -> u32 {
        self.r15
    }

    /// Force-align and write PC for the current ISA state (ARM: clear \[1:0\], Thumb: clear \[0\]).
    #[inline]
    pub fn set_pc(&mut self, addr: u32) {
        self.r15 = if self.thumb() { addr & !1 } else { addr & !3 };
    }

    /// `BX`-style interworking: bit0 selects Thumb; then force-align PC.
    ///
    /// This is the supported way to change the T bit (with exception entry/return).
    pub fn branch_exchange(&mut self, target: u32) {
        let thumb = (target & 1) != 0;
        self.set_thumb(thumb);
        self.r15 = if thumb { target & !1 } else { target & !3 };
    }

    #[inline]
    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    /// Replace CPSR. Illegal mode encodings keep the previous mode bits.
    /// Banking is dynamic, so mode changes need no register swap.
    pub fn set_cpsr(&mut self, value: u32) {
        let mode_bits = value & cpsr::MODE_MASK;
        let cpsr = if Mode::from_bits(mode_bits).is_some() {
            value
        } else {
            (value & !cpsr::MODE_MASK) | (self.cpsr & cpsr::MODE_MASK)
        };
        self.cpsr = cpsr;
    }

    /// Apply a PSR-transfer mask (flags/status/extension/control) onto CPSR.
    pub fn msr_cpsr(&mut self, value: u32, mask: u32) {
        let next = (self.cpsr & !mask) | (value & mask);
        self.set_cpsr(next);
    }

    /// Change only the mode field (leaves I/F/T/flags alone).
    pub fn set_mode(&mut self, mode: Mode) {
        self.cpsr = (self.cpsr & !cpsr::MODE_MASK) | mode.bits();
    }

    /// Current mode's SPSR, if any.
    pub fn spsr(&self) -> Option<u32> {
        self.mode().spsr_index().map(|i| self.spsr[i])
    }

    /// Write current mode's SPSR. No-op in User/System.
    pub fn set_spsr(&mut self, value: u32) {
        if let Some(i) = self.mode().spsr_index() {
            self.spsr[i] = value;
        }
    }

    /// Read SPSR for a specific mode (None for User/System).
    pub fn spsr_of(&self, mode: Mode) -> Option<u32> {
        mode.spsr_index().map(|i| self.spsr[i])
    }

    /// Write SPSR for a specific mode (no-op for User/System).
    pub fn set_spsr_of(&mut self, mode: Mode, value: u32) {
        if let Some(i) = mode.spsr_index() {
            self.spsr[i] = value;
        }
    }

    /// Exception-return style: copy SPSR → CPSR (mode/T/I/F/flags). No-op without SPSR.
    pub fn restore_cpsr_from_spsr(&mut self) {
        if let Some(spsr) = self.spsr() {
            self.set_cpsr(spsr);
        }
    }

    // --- Flag / control accessors -------------------------------------------------

    #[inline]
    pub fn n(&self) -> bool {
        self.cpsr & cpsr::N != 0
    }
    #[inline]
    pub fn z(&self) -> bool {
        self.cpsr & cpsr::Z != 0
    }
    #[inline]
    pub fn c(&self) -> bool {
        self.cpsr & cpsr::C != 0
    }
    #[inline]
    pub fn v(&self) -> bool {
        self.cpsr & cpsr::V != 0
    }
    #[inline]
    pub fn irq_disabled(&self) -> bool {
        self.cpsr & cpsr::I != 0
    }
    #[inline]
    pub fn fiq_disabled(&self) -> bool {
        self.cpsr & cpsr::F != 0
    }
    #[inline]
    pub fn thumb(&self) -> bool {
        self.cpsr & cpsr::T != 0
    }

    #[inline]
    pub fn set_nzcv(&mut self, n: bool, z: bool, c: bool, v: bool) {
        let mut p = self.cpsr & !(cpsr::N | cpsr::Z | cpsr::C | cpsr::V);
        if n {
            p |= cpsr::N;
        }
        if z {
            p |= cpsr::Z;
        }
        if c {
            p |= cpsr::C;
        }
        if v {
            p |= cpsr::V;
        }
        self.cpsr = p;
    }

    #[inline]
    pub fn set_irq_disabled(&mut self, disabled: bool) {
        if disabled {
            self.cpsr |= cpsr::I;
        } else {
            self.cpsr &= !cpsr::I;
        }
    }

    #[inline]
    pub fn set_fiq_disabled(&mut self, disabled: bool) {
        if disabled {
            self.cpsr |= cpsr::F;
        } else {
            self.cpsr &= !cpsr::F;
        }
    }

    /// Set Thumb bit. Prefer [`Self::branch_exchange`] / exception paths over MSR.
    #[inline]
    pub fn set_thumb(&mut self, thumb: bool) {
        if thumb {
            self.cpsr |= cpsr::T;
        } else {
            self.cpsr &= !cpsr::T;
        }
    }

    /// Evaluate an ARM condition field (`0b0000` EQ … `0b1110` AL).
    ///
    /// `0b1111` (NV) is reserved on ARMv3+; treated as never (false).
    pub fn cond_passed(&self, cond: u8) -> bool {
        let n = self.n();
        let z = self.z();
        let c = self.c();
        let v = self.v();
        match cond & 0xF {
            0x0 => z,            // EQ
            0x1 => !z,           // NE
            0x2 => c,            // CS/HS
            0x3 => !c,           // CC/LO
            0x4 => n,            // MI
            0x5 => !n,           // PL
            0x6 => v,            // VS
            0x7 => !v,           // VC
            0x8 => c && !z,      // HI
            0x9 => !c || z,      // LS
            0xA => n == v,       // GE
            0xB => n != v,       // LT
            0xC => !z && n == v, // GT
            0xD => z || n != v,  // LE
            0xE => true,         // AL
            0xF => false,        // NV (reserved)
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_svc_masked_arm() {
        let r = Regs::new();
        assert_eq!(r.mode(), Mode::Supervisor);
        assert!(r.irq_disabled());
        assert!(r.fiq_disabled());
        assert!(!r.thumb());
        assert_eq!(r.pc(), 0);
    }

    #[test]
    fn user_and_system_share_r13_r14() {
        let mut r = Regs::new();
        r.set_mode(Mode::User);
        r.set(13, 0x1111_0000);
        r.set(14, 0x2222_0000);
        r.set_mode(Mode::System);
        assert_eq!(r.get(13), 0x1111_0000);
        assert_eq!(r.get(14), 0x2222_0000);
        r.set(13, 0x3333_0000);
        r.set_mode(Mode::User);
        assert_eq!(r.get(13), 0x3333_0000);
    }

    #[test]
    fn irq_banks_sp_lr_only() {
        let mut r = Regs::new();
        r.set_mode(Mode::System);
        r.set(12, 0xC);
        r.set(13, 0x0300_7F00);
        r.set(14, 0xDEAD_BEEF);

        r.set_mode(Mode::Irq);
        assert_eq!(r.get(12), 0xC, "R12 not banked in IRQ");
        assert_eq!(r.get(13), 0, "fresh IRQ SP");
        assert_eq!(r.get(14), 0, "fresh IRQ LR");
        r.set(13, 0x0300_7FA0);
        r.set(14, 0x0800_0004);

        r.set_mode(Mode::System);
        assert_eq!(r.get(13), 0x0300_7F00);
        assert_eq!(r.get(14), 0xDEAD_BEEF);
        assert_eq!(r.get_r13_mode(Mode::Irq), 0x0300_7FA0);
        assert_eq!(r.get_r14_mode(Mode::Irq), 0x0800_0004);
    }

    #[test]
    fn fiq_banks_r8_through_r14() {
        let mut r = Regs::new();
        r.set_mode(Mode::User);
        for i in 8..=14 {
            r.set(i, 0x1000 + u32::from(i));
        }

        r.set_mode(Mode::Fiq);
        for i in 8..=14 {
            assert_eq!(r.get(i), 0, "FIQ bank starts clear");
            r.set(i, 0xF000 + u32::from(i));
        }
        // R0–R7 still shared
        r.set(0, 42);
        assert_eq!(r.get(0), 42);

        r.set_mode(Mode::User);
        for i in 8..=14 {
            assert_eq!(r.get(i), 0x1000 + u32::from(i));
        }
        r.set_mode(Mode::Fiq);
        for i in 8..=14 {
            assert_eq!(r.get(i), 0xF000 + u32::from(i));
        }
    }

    #[test]
    fn spsr_per_exception_mode() {
        let mut r = Regs::new();
        r.set_mode(Mode::Irq);
        r.set_spsr(0x6000_0010); // User + some flags
        r.set_mode(Mode::Supervisor);
        r.set_spsr(0x8000_0013);
        assert_eq!(r.spsr_of(Mode::Irq), Some(0x6000_0010));
        assert_eq!(r.spsr(), Some(0x8000_0013));

        r.set_mode(Mode::User);
        assert!(r.spsr().is_none());
        r.set_spsr(0x1234); // no-op
        assert!(r.spsr_of(Mode::User).is_none());
    }

    #[test]
    fn restore_cpsr_from_spsr_switches_mode() {
        let mut r = Regs::new();
        r.set_mode(Mode::Irq);
        r.set_spsr(Mode::System.bits() | cpsr::N); // System, N set, I/F/T clear
        r.restore_cpsr_from_spsr();
        assert_eq!(r.mode(), Mode::System);
        assert!(r.n());
        assert!(!r.irq_disabled());
        assert!(!r.thumb());
    }

    #[test]
    fn branch_exchange_sets_t_and_aligns() {
        let mut r = Regs::new();
        r.branch_exchange(0x0800_0001);
        assert!(r.thumb());
        assert_eq!(r.pc(), 0x0800_0000);

        r.branch_exchange(0x0800_0002);
        assert!(!r.thumb());
        assert_eq!(r.pc(), 0x0800_0000);

        r.set_thumb(true);
        r.set_pc(0x0800_0003);
        assert_eq!(r.pc(), 0x0800_0002);
    }

    #[test]
    fn illegal_mode_bits_rejected() {
        let mut r = Regs::new();
        r.set_mode(Mode::Irq);
        r.set_cpsr(0x8000_0014); // illegal mode 0x14
        assert_eq!(r.mode(), Mode::Irq);
        assert!(r.n());
    }

    #[test]
    fn cond_codes() {
        let mut r = Regs::new();
        r.set_nzcv(false, true, false, false); // Z
        assert!(r.cond_passed(0x0)); // EQ
        assert!(!r.cond_passed(0x1)); // NE
        assert!(r.cond_passed(0xE)); // AL
        assert!(!r.cond_passed(0xF)); // NV

        r.set_nzcv(true, false, true, true); // N C V
        assert!(r.cond_passed(0xA)); // GE (N==V)
        assert!(r.cond_passed(0x8)); // HI (C && !Z)
    }

    #[test]
    fn msr_control_mask_can_change_mode() {
        let mut r = Regs::new();
        r.set_mode(Mode::Supervisor);
        assert!(r.irq_disabled());
        // MSR CPSR_c with System mode bits clears I/F/T in the control byte.
        r.msr_cpsr(Mode::System.bits(), cpsr::CONTROL_MASK);
        assert_eq!(r.mode(), Mode::System);
        assert!(!r.irq_disabled());
        assert!(!r.fiq_disabled());
        assert!(!r.thumb());
    }
}
