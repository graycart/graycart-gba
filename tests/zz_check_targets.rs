use graycart_gba::Machine;

fn check(rel: &str) {
    let data = std::fs::read(format!("tests/fixtures/alyosha/{rel}.gba")).unwrap();
    let mut m = Machine::from_rom(data);
    m.run_frames(60);
    let r12 = m.cpu.reg(12);
    let pass = m.idle && m.error.is_none() && r12 == 0;
    eprintln!(
        "{:<40} {} r12={} idle={} err={:?} pc={:08X}",
        rel,
        if pass { "PASS" } else { "FAIL" },
        r12,
        m.idle,
        m.error,
        m.cpu.exec_pc
    );
}

#[test]
fn check_targets() {
    for rel in [
        "DMA/DMA_ROM_Fixed",
        "DMA/DMA_pause_timing_end_2",
        "timer/timer",
        "timer/timer_reset",
        "timer/timer_disable",
    ] {
        check(rel);
    }
}
