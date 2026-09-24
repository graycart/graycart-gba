//! Owned machine instance on the emulation thread.

use crate::compat::{AGB_A, AGB_B};
use crate::hw::Machine as ArmMachine;
use graycart::{Bus, Cpu, ExecSession, apply_fast};
use std::path::{Path, PathBuf};

pub(super) enum Machine {
    Arm {
        path: PathBuf,
        save_path: PathBuf,
        title: String,
        inner: Box<ArmMachine>,
    },
    Sm83 {
        path: PathBuf,
        save_path: PathBuf,
        title: String,
        cpu: Cpu,
        bus: Box<Bus>,
        session: ExecSession,
    },
}

impl Machine {
    #[allow(dead_code)]
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Arm { path, .. } | Self::Sm83 { path, .. } => path,
        }
    }

    #[allow(dead_code)]
    pub(super) fn save_path(&self) -> &Path {
        match self {
            Self::Arm { save_path, .. } | Self::Sm83 { save_path, .. } => save_path,
        }
    }

    pub(super) fn title(&self) -> &str {
        match self {
            Self::Arm { title, .. } | Self::Sm83 { title, .. } => title,
        }
    }

    #[allow(dead_code)]
    pub(super) fn is_arm(&self) -> bool {
        matches!(self, Self::Arm { .. })
    }

    pub(super) fn apply_agb_regs(cpu: &mut Cpu) {
        cpu.a = AGB_A;
        cpu.b = AGB_B;
    }
}

pub(super) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

#[allow(dead_code)]
pub(super) fn rom_title(cart: &graycart::Cartridge, path: &Path) -> String {
    let t = cart.header.title.trim();
    if t.is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ROM")
            .to_string()
    } else {
        t.to_string()
    }
}

pub(super) fn sm83_from_cart(
    path: PathBuf,
    save_path: PathBuf,
    title: String,
    cart: graycart::Cartridge,
    hardware_pref: graycart::HostHardwarePref,
) -> Result<Machine, String> {
    let bus = graycart::bus_from_cartridge(cart, hardware_pref).map_err(|e| e.to_string())?;
    let mut bus = Box::new(bus);
    let mut cpu = Cpu::new();
    apply_fast(&mut cpu, &mut bus);
    Machine::apply_agb_regs(&mut cpu);
    Ok(Machine::Sm83 {
        path,
        save_path,
        title,
        cpu,
        bus,
        session: ExecSession::new(),
    })
}
