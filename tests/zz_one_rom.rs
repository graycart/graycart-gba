use graycart_gba::Machine;
#[test]
fn one() {
    let data = std::fs::read("tests/fixtures/alyosha/DMA/DMA_ROM_Fixed.gba").unwrap();
    let mut m = Machine::from_rom(data);
    m.run_frames(60);
    eprintln!("DONE r12={} ctr={:04X}", m.cpu.reg(12), m.bus.timers.counter(0));
}
