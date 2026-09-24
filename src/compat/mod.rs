//! Game Boy handoff. This crate owns the switch. The SM83 machine is `graycart`.

use std::path::Path;

use graycart::{
    Cartridge, Cpu, ExecSession, HostHardwarePref, RunOutcome, apply_fast, bus_from_cartridge,
};

/// AGB / AGB0 boot registers (Pan Docs power-up sequence).
pub const AGB_A: u8 = 0x11;
pub const AGB_B: u8 = 0x01;

/// Pan Docs mixes CGB audio digitally. GBATEK still calls the four channels analogue.
/// This page does not pick a side. The Game Boy machine keeps the `graycart` APU.
pub const CGB_AUDIO_UNIMPLEMENTED: &str =
    "gba-debug: warn cgb-audio pandocs-vs-gbatek unimplemented keep=graycart-apu";

/// Result of handing a `.gb` / `.gbc` image to `graycart`.
pub struct Handoff {
    pub frames: u64,
    pub a: u8,
    pub b: u8,
    /// ARM opcodes executed on this path. Always 0: the ARM core is not in the loop.
    pub arm_opcodes: u32,
}

/// Run `frames` on the SM83. Does not construct or step an ARM7.
pub fn run_sm83(bytes: &[u8], frames: u32) -> Result<Handoff, String> {
    let cart = Cartridge::rom_only(bytes.to_vec());
    run_cart(cart, frames)
}

/// Load a `.gb` / `.gbc` file and run it on the SM83.
pub fn run_sm83_file(path: &Path, frames: u32) -> Result<Handoff, String> {
    let cart = Cartridge::load(path).map_err(|err| err.to_string())?;
    run_cart(cart, frames)
}

fn run_cart(cart: Cartridge, frames: u32) -> Result<Handoff, String> {
    // The SM83 bus is larger than the Windows main-thread stack.
    std::thread::Builder::new()
        .name("sm83".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || run_cart_on_thread(cart, frames))
        .map_err(|err| err.to_string())?
        .join()
        .unwrap_or_else(|_| Err("sm83 thread panicked".to_string()))
}

fn run_cart_on_thread(cart: Cartridge, frames: u32) -> Result<Handoff, String> {
    let mut bus =
        bus_from_cartridge(cart, HostHardwarePref::Automatic).map_err(|err| err.to_string())?;
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);
    cpu.a = AGB_A;
    cpu.b = AGB_B;
    let mut session = ExecSession::new();
    match session.run_frames(&mut cpu, &mut bus, u64::from(frames)) {
        RunOutcome::FrameLimit { frames, .. } => Ok(Handoff {
            frames,
            a: AGB_A,
            b: AGB_B,
            arm_opcodes: 0,
        }),
        RunOutcome::Fault(report) => Err(format!("sm83: {report}")),
    }
}

pub fn machine_line(kind: &str, handoff: Option<&Handoff>) -> String {
    match handoff {
        Some(handoff) => format!(
            "gba-debug: machine={kind} a={:02X} b={:02X}",
            handoff.a, handoff.b
        ),
        None => format!("gba-debug: machine={kind}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    fn nop_rom(cgb: u8) -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[0x0143] = cgb;
        rom
    }

    #[test]
    fn gb_and_gbc_reach_a_frame_without_arm() {
        for cgb in [0x00, 0xC0] {
            let handoff = run_sm83(&nop_rom(cgb), 1).expect("sm83 frame");
            assert!(handoff.frames >= 1);
            assert_eq!(handoff.a, AGB_A);
            assert_eq!(handoff.b, AGB_B);
            assert_eq!(handoff.arm_opcodes, 0);
        }
    }

    #[test]
    fn stop_applies_the_switch_and_arm_does_not_step() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        machine.bus.set_gb_cart_shape(true);
        assert_eq!(machine.bus.read16(0x0400_0204) & 0x8000, 0x8000);
        machine.bus.write16(0x0400_0204, 0);
        assert_eq!(machine.bus.read16(0x0400_0204) & 0x8000, 0x8000);
        machine.bus.write16(0x0400_0000, 0x0008);
        machine.bus.write8(0x0400_0301, 0x80);
        assert!(machine.bus.gb_mode());
        let pc = machine.cpu.exec_pc;
        machine.run_frames(1);
        assert_eq!(machine.cpu.exec_pc, pc);
        assert_eq!(machine.arm_steps, 0);
    }

    #[test]
    fn halt_without_prepare_does_not_switch() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        machine.bus.set_gb_cart_shape(true);
        machine.bus.write8(0x0400_0301, 0);
        assert!(!machine.bus.gb_mode());
        assert!(machine.bus.halted);
    }

    #[test]
    fn cgb_romdis_is_bit_3_at_4000800() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        assert_eq!(machine.bus.read8(0x0400_0800), 0);
        machine.bus.write8(0x0400_0800, 0x08);
        assert!(machine.bus.cgb_romdis());
        assert_eq!(machine.bus.read8(0x0400_0800), 0x08);
        machine.bus.write8(0x0400_0800, 0);
        assert!(!machine.bus.cgb_romdis());
    }
}
