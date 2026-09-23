use graycart_gba::{format_trace_line, MachineDebug};

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
fn trace_line_uses_the_debug_prefix() {
    let line = format_trace_line(0x0800_0000, "nop", 0, 0x1F);
    assert_eq!(
        line,
        "gba-debug: trace pc=0x08000000 mnemonic=nop r12=0 cpsr=0x0000001F"
    );
}
