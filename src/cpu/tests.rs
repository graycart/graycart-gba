use super::{Cpu, StepError};

#[test]
fn unimplemented_names_the_opcode() {
    let err = StepError::Unimplemented {
        pc: 0x0800_0000,
        mnemonic: "arm 0xE7F000F0".into(),
    };
    assert_eq!(
        err.to_string(),
        "unimplemented pc=0x08000000 mnemonic=arm 0xE7F000F0"
    );
}

#[test]
fn reset_is_system_mode_at_rom() {
    let cpu = Cpu::new();
    assert_eq!(cpu.cpsr(), 0x1F);
    assert_eq!(cpu.exec_pc, 0x0800_0000);
    assert_eq!(cpu.reg(13), 0x0300_7F00);
    assert!(!cpu.idle);
}
