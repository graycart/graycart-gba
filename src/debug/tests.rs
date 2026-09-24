use super::hash::sha256_hex;
use super::{
    MachineDebug, cpu_result_line, format_trace_line, frame_hash, frame_nonzero, live_cpu_line,
    summary_frames,
};

#[test]
fn stub_summary_marks_sections_absent_and_keeps_apu_health() {
    let lines = MachineDebug::absent().summary_lines(1);
    let text = lines.join("\n");
    assert!(text.contains("gba-debug: cpu frame=1 absent"));
    assert!(text.contains("gba-debug: ppu frame=1 absent"));
    assert!(text.contains("gba-debug: dma summary frame=1 absent"));
    assert!(text.contains("gba-debug: irq summary frame=1 absent"));
    assert!(text.contains("gba-debug: ppu health frame=1 absent"));
    assert!(text.contains("gba-debug: apu host frame=1 absent"));
    assert!(text.contains("gba-debug: swi summary frame=1 absent"));
    assert!(text.contains(
        "gba-debug: apu health frame=1 master=0 pwm=0Hz fifoA=absent underrun=0/0 overrun=0/0 empty=0/0 lag=0/0 dma_req=0/0 peak=[0..0] dc≈[0,0] clip=0 extreme=0 psg_on=0x0 psg_nr50=0x00 fifoB=absent"
    ));
    assert!(!text.to_lowercase().contains("unimplemented"));
}

#[test]
fn av_report_is_empty_sections() {
    let report = MachineDebug::absent().av_report(1);
    assert!(report.starts_with("=== graycart-gba AV report (frame=1) ===\n"));
    assert!(report.contains("PPU absent"));
    assert!(report.contains("APU master=0 pwm=0Hz fifoA=absent fifoB=absent underrun=0/0 overrun=0/0 empty_drain=0/0 peak=[0..0] dc≈[0,0] clip=0"));
    assert!(report.contains("SWI unhandled=(none)"));
}

#[test]
fn summary_hits_every_60_and_the_last_frame() {
    assert_eq!(summary_frames(1), vec![1]);
    assert_eq!(summary_frames(60), vec![60]);
    assert_eq!(summary_frames(61), vec![60, 61]);
}

#[test]
fn trace_line_uses_the_debug_prefix() {
    let line = format_trace_line(0x0800_0000, "nop", 0, 0x1F);
    assert_eq!(
        line,
        "gba-debug: trace pc=0x08000000 mnemonic=nop r12=0 cpsr=0x0000001F"
    );
}

#[test]
fn cpu_result_line_prints_both_registers() {
    assert_eq!(
        cpu_result_line(0, 0, 0x0800_0000, "b", true),
        "gba-debug: cpu result=PASS r12=0 r7=0"
    );
    assert_eq!(
        cpu_result_line(0, 50349563, 0x0800_1EC4, "b", true),
        "gba-debug: cpu result=PASS r12=0 r7=50349563"
    );
    assert_eq!(
        cpu_result_line(0, 220, 0x0800_07EE, "b", true),
        "gba-debug: cpu result=FAIL r12=0 r7=220 pc=0x080007EE mnemonic=b"
    );
}

#[test]
fn live_cpu_line_power_halt_when_halted_not_idle() {
    let line = live_cpu_line(1, 0x0800_0000, 0x1F, false, true, false, 0, 0);
    assert!(line.contains("power=Halt"), "{line}");
    let run = live_cpu_line(1, 0x0800_0000, 0x1F, false, false, false, 0, 0);
    assert!(run.contains("power=Run"), "{run}");
}

#[test]
fn sha256_hex_abc_known_answer() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn frame_hash_one_pixel_le_bytes() {
    assert_eq!(frame_hash(&[0x001F]), sha256_hex(&[0x1F, 0x00]));
}

#[test]
fn frame_nonzero_ignores_high_bit_alone() {
    assert_eq!(frame_nonzero(&[0, 0x8000, 1]), 1);
}
