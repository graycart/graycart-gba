use super::*;
use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use tempfile::tempdir;

#[cfg(unix)]
struct ReadOnlyBootDir(PathBuf);

#[cfg(unix)]
impl ReadOnlyBootDir {
    fn new(path: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let boot_dir = path.parent().unwrap().to_path_buf();
        let mut perms = fs::metadata(&boot_dir).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&boot_dir, perms).unwrap();
        Self(boot_dir)
    }
}

#[cfg(unix)]
impl Drop for ReadOnlyBootDir {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&self.0).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&self.0, perms).unwrap();
    }
}

#[cfg(windows)]
fn exclusive_boot_rom_lock(path: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .unwrap()
}

#[test]
fn rejects_non_256() {
    let dir = tempdir().unwrap();
    assert!(install_boot_rom(dir.path(), &[0u8; 255]).is_err());
    assert!(install_boot_rom(dir.path(), &[0u8; 257]).is_err());
}

#[test]
fn install_load_clear_round_trip() {
    let dir = tempdir().unwrap();
    let a = [0xAAu8; 256];
    install_boot_rom(dir.path(), &a).unwrap();
    assert_eq!(load_cached_boot_rom(dir.path()).unwrap(), a);
    clear_boot_rom(dir.path()).unwrap();
    assert!(load_cached_boot_rom(dir.path()).is_none());
}

#[test]
fn failed_replace_preserves_previous() {
    let dir = tempdir().unwrap();
    let exe_dir = dir.path();
    let a = [0xAAu8; 256];
    let b = [0xBBu8; 256];
    install_boot_rom(exe_dir, &a).unwrap();

    let cache_path = boot_rom_cache_path(exe_dir);
    #[cfg(windows)]
    let lock = exclusive_boot_rom_lock(&cache_path);
    #[cfg(unix)]
    let _guard = ReadOnlyBootDir::new(&cache_path);

    assert!(install_boot_rom(exe_dir, &b).is_err());

    #[cfg(windows)]
    drop(lock);

    assert_eq!(load_cached_boot_rom(exe_dir).unwrap(), a);
}

#[test]
fn load_rejects_wrong_size_on_disk() {
    let dir = tempdir().unwrap();
    let path = boot_rom_cache_path(dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();

    fs::write(&path, [0u8; 255]).unwrap();
    assert!(load_cached_boot_rom(dir.path()).is_none());

    fs::write(&path, [0u8; 512]).unwrap();
    assert!(load_cached_boot_rom(dir.path()).is_none());
}

#[test]
fn path_ends_with_boot_dmg_boot_bin() {
    let p = boot_rom_cache_path(Path::new("/fake/graycart"));
    assert!(
        p.ends_with("boot/dmg_boot.bin") || p.ends_with(r"boot\dmg_boot.bin"),
        "unexpected path: {p:?}"
    );
}

#[test]
fn install_boot_rom_rejects_2048() {
    let dir = tempdir().unwrap();
    assert!(install_boot_rom(dir.path(), &[0u8; 2048]).is_err());
}

#[test]
fn rejects_non_2048_cgb() {
    let dir = tempdir().unwrap();
    assert!(install_cgb_boot_rom(dir.path(), &[0u8; 2047]).is_err());
    assert!(install_cgb_boot_rom(dir.path(), &[0u8; 256]).is_err());
    assert!(install_cgb_boot_rom(dir.path(), &[0u8; 2049]).is_err());
}

#[test]
fn install_load_cgb_round_trip() {
    let dir = tempdir().unwrap();
    let a = [0xCCu8; 2048];
    install_cgb_boot_rom(dir.path(), &a).unwrap();
    assert_eq!(*load_cached_cgb_boot_rom(dir.path()).unwrap(), a);
    assert!(load_cached_boot_rom(dir.path()).is_none());
    assert!(any_cached_boot_firmware(dir.path()));
    clear_boot_rom(dir.path()).unwrap();
    assert!(load_cached_cgb_boot_rom(dir.path()).is_none());
    assert!(!any_cached_boot_firmware(dir.path()));
}

#[test]
fn clear_removes_dmg_and_cgb() {
    let dir = tempdir().unwrap();
    install_boot_rom(dir.path(), &[0xAAu8; 256]).unwrap();
    install_cgb_boot_rom(dir.path(), &[0xCCu8; 2048]).unwrap();
    assert!(any_cached_boot_firmware(dir.path()));
    clear_boot_rom(dir.path()).unwrap();
    assert!(load_cached_boot_rom(dir.path()).is_none());
    assert!(load_cached_cgb_boot_rom(dir.path()).is_none());
}

#[test]
fn load_cgb_rejects_wrong_size_on_disk() {
    let dir = tempdir().unwrap();
    let path = cgb_boot_rom_cache_path(dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();

    fs::write(&path, [0u8; 256]).unwrap();
    assert!(load_cached_cgb_boot_rom(dir.path()).is_none());

    fs::write(&path, [0u8; 2047]).unwrap();
    assert!(load_cached_cgb_boot_rom(dir.path()).is_none());
}

#[test]
fn any_cached_uses_size_not_full_read() {
    let dir = tempdir().unwrap();
    let path = boot_rom_cache_path(dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, [0u8; 255]).unwrap();
    assert!(!any_cached_boot_firmware(dir.path()));
    fs::write(&path, [0xAAu8; 256]).unwrap();
    assert!(any_cached_boot_firmware(dir.path()));
}

#[test]
fn path_ends_with_boot_cgb_boot_bin() {
    let p = cgb_boot_rom_cache_path(Path::new("/fake/graycart"));
    assert!(
        p.ends_with("boot/cgb_boot.bin") || p.ends_with(r"boot\cgb_boot.bin"),
        "unexpected path: {p:?}"
    );
}
