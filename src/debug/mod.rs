//! Debug text. Lines are `gba-debug:` and `key=value`.
//!
//! Cited: the previous graycart-gba console log shape, reimplemented here.
//! The deleted `src/debug` module was not copied.

mod hash;

pub use hash::{frame_hash, frame_nonzero};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineDebug;

impl MachineDebug {
    pub fn absent() -> Self {
        Self
    }

    pub fn summary_lines(&self, frame: u32) -> Vec<String> {
        vec![
            format!("gba-debug: cpu frame={frame} absent"),
            format!("gba-debug: ppu frame={frame} absent"),
            format!("gba-debug: dma summary frame={frame} absent"),
            format!("gba-debug: irq summary frame={frame} absent"),
            format!("gba-debug: ppu health frame={frame} absent"),
            format!("gba-debug: apu host frame={frame} absent"),
            format!("gba-debug: swi summary frame={frame} absent"),
            format!(
                "gba-debug: apu health frame={frame} master=0 pwm=0Hz fifoA=absent underrun=0/0 overrun=0/0 empty=0/0 lag=0/0 dma_req=0/0 peak=[0..0] dc≈[0,0] clip=0 extreme=0 psg_on=0x0 psg_nr50=0x00 fifoB=absent"
            ),
        ]
    }

    pub fn av_report(&self, frame: u32) -> String {
        format!(
            "\
=== graycart-gba AV report (frame={frame}) ===
PPU absent
PPU pixels absent
APU master=0 pwm=0Hz fifoA=absent fifoB=absent underrun=0/0 overrun=0/0 empty_drain=0/0 peak=[0..0] dc≈[0,0] clip=0
SWI unhandled=(none)
"
        )
    }

    /// Live AV report for a ROM that has a settled framebuffer.
    pub fn av_report_live(&self, frame: u32, dispcnt: u16, pixels: &[u16], nonzero: u32) -> String {
        let mode = dispcnt & 7;
        let blank = u8::from(dispcnt & (1 << 7) != 0);
        let hash = frame_hash(pixels);
        format!(
            "\
=== graycart-gba AV report (frame={frame}) ===
PPU mode={mode} dispcnt=0x{dispcnt:04X} blank={blank} hash={hash}
PPU pixels nonzero={nonzero}
APU master=0 pwm=0Hz fifoA=absent fifoB=absent underrun=0/0 overrun=0/0 empty_drain=0/0 peak=[0..0] dc≈[0,0] clip=0
SWI unhandled=(none)
"
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub fn live_cpu_line(
    frame: u32,
    pc: u32,
    cpsr: u32,
    idle: bool,
    halted: bool,
    ime: bool,
    ie: u16,
    iff: u16,
) -> String {
    let thumb = cpsr & 0x20 != 0;
    let i_mask = cpsr & 0x80 != 0;
    let mode = match cpsr & 0x1F {
        0x10 => "User",
        0x11 => "Fiq",
        0x12 => "Irq",
        0x13 => "Svc",
        0x17 => "Abt",
        0x1B => "Und",
        0x1F => "System",
        _ => "Inv",
    };
    let power = if idle || halted { "Halt" } else { "Run" };
    format!(
        "gba-debug: cpu frame={frame} pc={pc:#010X} region={} cpsr={cpsr:#010X} mode={mode} thumb={thumb} i_mask={i_mask} ime={} ie=0x{ie:04X} if=0x{iff:04X} power={power}",
        region(pc),
        u8::from(ime),
    )
}

pub fn cpu_result_line(r12: u32, r7: u32, pc: u32, op: &str, idle: bool) -> String {
    // r7 in 1..=999 is a thumb.gba failure id; arm.gba's leftover r7 is outside that
    // range, so the PASS word follows r12 unless r7 is a small id.
    let thumb_fail_id = (1..=999).contains(&r7);
    if idle && r12 == 0 && !thumb_fail_id {
        format!("gba-debug: cpu result=PASS r12=0 r7={r7}")
    } else {
        format!("gba-debug: cpu result=FAIL r12={r12} r7={r7} pc={pc:#010X} mnemonic={op}")
    }
}

fn region(pc: u32) -> &'static str {
    match pc >> 24 {
        0x00 => "bios",
        0x02 => "ewram",
        0x03 => "iwram",
        0x04 => "io",
        0x05 => "pal",
        0x06 => "vram",
        0x07 => "oam",
        0x08..=0x0D => "rom",
        0x0E => "sram",
        _ => "open",
    }
}

pub fn format_trace_line(pc: u32, mnemonic: &str, r12: u32, cpsr: u32) -> String {
    format!("gba-debug: trace pc={pc:#010X} mnemonic={mnemonic} r12={r12} cpsr={cpsr:#010X}")
}

/// Frames that get a summary line: every 60th, and the last frame of the run.
pub fn summary_frames(frames: u32) -> Vec<u32> {
    if frames == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut frame = 60;
    while frame < frames {
        out.push(frame);
        frame += 60;
    }
    out.push(frames);
    out
}

#[cfg(test)]
mod tests;
