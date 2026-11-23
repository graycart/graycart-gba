//! Metroid Fusion boot smoke (carts/ gitignored dump).

use graycart_gba::Machine;

#[test]
fn metroid_fusion_iwram_stub_and_past_force_blank() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../carts/Metroid Fusion (USA).gba");
    if !path.exists() {
        return;
    }
    let mut m = Machine::open(&path).expect("open");
    m.run_frames(12);
    assert_eq!(m.bus.read16(0x0300_3CB4), 0xB4F0, "IWRAM stub halfword 0");
    assert_eq!(m.bus.read16(0x0300_3CB6), 0x4644, "IWRAM stub halfword 1");
    assert!(m.error.is_none(), "early fault: {:?}", m.error);

    let mut cleared_blank = false;
    for _ in 0..200u32 {
        m.run_frames(1);
        if m.error.is_some() {
            break;
        }
        if m.bus.read16(0x0400_0000) & 0x80 == 0 {
            cleared_blank = true;
            break;
        }
    }
    assert!(
        cleared_blank && m.error.is_none(),
        "force blank stuck: DISPCNT={:#X} pc={:#X} err={:?}",
        m.bus.read16(0x0400_0000),
        m.cpu.exec_pc,
        m.error
    );
}
