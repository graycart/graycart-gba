//! Chooses ARM (GBA) vs SM83 (Game Boy / Color) from the path extension only.
//! ROM header inspection happens later in the `graycart` machine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineKind {
    Arm,
    Sm83,
}

pub fn machine_kind(path: &std::path::Path) -> Option<MachineKind> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("gba") => Some(MachineKind::Arm),
        Some("gb") | Some("gbc") => Some(MachineKind::Sm83),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn machine_kind_gba_is_arm() {
        assert_eq!(machine_kind(Path::new("file.gba")), Some(MachineKind::Arm));
    }

    #[test]
    fn machine_kind_gba_uppercase_is_arm() {
        assert_eq!(machine_kind(Path::new("FILE.GBA")), Some(MachineKind::Arm));
    }

    #[test]
    fn machine_kind_gb_is_sm83() {
        assert_eq!(machine_kind(Path::new("file.gb")), Some(MachineKind::Sm83));
    }

    #[test]
    fn machine_kind_gbc_is_sm83() {
        assert_eq!(machine_kind(Path::new("file.gbc")), Some(MachineKind::Sm83));
    }

    #[test]
    fn machine_kind_gbc_uppercase_is_sm83() {
        assert_eq!(machine_kind(Path::new("file.GBC")), Some(MachineKind::Sm83));
    }

    #[test]
    fn machine_kind_bin_is_none() {
        assert_eq!(machine_kind(Path::new("file.bin")), None);
    }

    #[test]
    fn machine_kind_no_extension_is_none() {
        assert_eq!(machine_kind(Path::new("file")), None);
    }
}
