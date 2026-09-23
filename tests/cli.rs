use std::fs;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graycart-gba"))
}

fn output(cmd: &mut Command) -> std::process::Output {
    cmd.output().expect("spawn graycart-gba")
}

#[test]
fn no_path_exits_with_a_clear_error() {
    let mut cmd = bin();
    let out = output(&mut cmd);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("graycart-gba <rom-or-directory> --frames <n> [--debug]"));
    assert!(err.contains("graycart-gba <rom-or-directory> --debug [--frames <n>]"));
    assert!(!err.to_lowercase().contains("unimplemented"));
}

#[test]
fn tiny_file_debug_prints_absent_sections_and_the_log() {
    let dir = std::env::temp_dir().join(format!("graycart-gba-page0-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("tiny.gba");
    fs::write(&rom, [0u8, 1, 2, 3]).unwrap();

    let mut cmd = bin();
    let out = output(
        cmd.current_dir(&dir)
            .arg(&rom)
            .arg("--frames")
            .arg("1")
            .arg("--debug"),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(err.contains("gba-debug: cpu frame=1 absent"));
    assert!(err.contains("gba-debug: apu health frame=1 master=0"));
    assert!(stdout.contains("=== graycart-gba AV report (frame=1) ==="));
    assert!(stdout.contains("PPU absent"));
    assert!(!err.to_lowercase().contains("unimplemented"));

    let log = fs::read_to_string(dir.join("gba-debug.log")).unwrap();
    assert!(log.contains("gba-debug: cpu frame=1 absent"));
    assert!(log.contains("=== graycart-gba AV report (frame=1) ==="));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn debug_spin_exits_nonzero_on_fail() {
    let dir = std::env::temp_dir().join(format!("graycart-gba-spin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut rom = vec![0u8; 0xC0];
    rom[0..4].copy_from_slice(&0xE280_0001u32.to_le_bytes());
    rom[4..8].copy_from_slice(&0xEAFF_FFFDu32.to_le_bytes());
    let path = dir.join("spin.gba");
    fs::write(&path, &rom).unwrap();

    let mut cmd = bin();
    let out = output(cmd.current_dir(&dir).arg(&path).arg("--debug"));
    assert!(
        !out.status.success(),
        "fail must exit non-zero\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("result=FAIL"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn arm_rom_without_frames_stops_on_pass() {
    let rom = std::path::Path::new("tests/fixtures/jsmolka/arm/arm.gba");
    if !rom.exists() {
        eprintln!("skip arm.gba");
        return;
    }
    let mut cmd = bin();
    let out = output(cmd.arg(rom).arg("--debug"));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("gba-debug: cpu result=PASS r12=0"), "{err}");
}

#[test]
fn debug_without_frames_still_prints_the_report() {
    let dir = std::env::temp_dir().join(format!(
        "graycart-gba-page0-noframes-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("tiny.gba");
    fs::write(&rom, [0u8, 1, 2, 3]).unwrap();

    let mut cmd = bin();
    let out = output(cmd.current_dir(&dir).arg(&rom).arg("--debug"));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("=== graycart-gba AV report (frame=1) ==="));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_directory_is_an_error() {
    let dir = std::env::temp_dir().join(format!("graycart-gba-page0-empty-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut cmd = bin();
    let out = output(cmd.arg(&dir).arg("--frames").arg("1").arg("--debug"));
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no carts"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn directory_smoke_is_not_pass_or_fail() {
    let dir = std::env::temp_dir().join(format!("graycart-gba-page0-dir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.gba"), [0u8]).unwrap();
    fs::write(dir.join("notes.txt"), b"skip").unwrap();
    let work = dir.join("work");
    fs::create_dir_all(&work).unwrap();

    let mut cmd = bin();
    let out = output(
        cmd.current_dir(&work)
            .arg(&dir)
            .arg("--frames")
            .arg("1")
            .arg("--debug"),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("gba-debug: smoke file=a.gba frames=1 fault=none"));
    assert!(!err.contains("result=PASS"));
    assert!(!err.contains("result=FAIL"));
    assert!(!err.contains("notes.txt"));
    let _ = fs::remove_dir_all(&dir);
}
