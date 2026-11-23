//! Smoke-test commercial ROMs under `../carts` (skipped if the folder is empty).

use graycart_gba::{Machine, cart};

fn carts_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../carts")
}

fn list_roms() -> Vec<std::path::PathBuf> {
    let dir = carts_dir();
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut v: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("gba"))
        .collect();
    v.sort();
    v
}

#[derive(Debug)]
struct CartReport {
    name: String,
    save: &'static str,
    rtc: bool,
    frames: u32,
    nonzero: u32,
    gpio_reject: bool,
    fault: Option<String>,
}

fn smoke_one(path: &std::path::Path, frames: u32) -> CartReport {
    let name = path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let bytes = std::fs::read(path).expect("read");
    let save = cart::detect_save(&bytes);
    let rtc = cart::detect_rtc(&bytes);
    let mut machine = Machine::open(path).expect("open");
    assert_eq!(machine.bus.save_kind(), save, "{name} save kind");
    assert_eq!(machine.bus.has_rtc(), rtc, "{name} rtc");
    machine.run_frames(frames);
    let gpio_reject = machine
        .bus
        .warn_lines
        .iter()
        .any(|l| l.contains("gpio unsupported"));
    CartReport {
        name,
        save: save.name(),
        rtc,
        frames: machine.frames_done(),
        nonzero: machine.ppu.nonzero(),
        gpio_reject,
        fault: machine.error.as_ref().map(|e| e.to_string()),
    }
}

#[test]
fn cart_folder_inventory_and_boot() {
    let roms = list_roms();
    if roms.is_empty() {
        eprintln!("no carts; skip");
        return;
    }
    let mut reports = Vec::new();
    for path in &roms {
        let report = smoke_one(path, 240);
        println!(
            "CART {}: save={} rtc={} frames={} nonzero={} gpio_reject={} fault={:?}",
            report.name,
            report.save,
            report.rtc,
            report.frames,
            report.nonzero,
            report.gpio_reject,
            report.fault
        );
        reports.push(report);
    }

    for r in &reports {
        assert!(
            !r.gpio_reject,
            "{}: GPIO reject warn must not fire (RTC carts use real GPIO)",
            r.name
        );
    }

    // Pokemon: RTC must work; no CPU fault in early boot.
    for r in reports.iter().filter(|r| r.rtc) {
        assert!(
            r.fault.is_none(),
            "{}: RTC cart must not CPU-fault: {:?}",
            r.name,
            r.fault
        );
    }

    // Advance Wars 2: should keep running and draw something.
    if let Some(aw2) = reports
        .iter()
        .find(|r| r.name.contains("Advance Wars 2"))
    {
        assert!(aw2.fault.is_none(), "AW2 fault: {:?}", aw2.fault);
        assert!(aw2.nonzero > 0, "AW2 expected pixels, got 0");
        assert_eq!(aw2.save, "flash64");
    }

    // Metroid Fusion: separate boot crash (not RTC). Keep inventory coverage only.
    if let Some(fusion) = reports.iter().find(|r| r.name.contains("Metroid Fusion")) {
        assert_eq!(fusion.save, "sram");
        if let Some(fault) = &fusion.fault {
            println!("NOTE Metroid Fusion still faults early: {fault}");
        }
    }
}

#[test]
fn emerald_and_sapphire_status_power_clear() {
    let mut checked = 0;
    for rel in [
        "Pokemon - Emerald Version (USA, Europe).gba",
        "Pokemon - Sapphire Version (USA, Europe) (Rev 2).gba",
    ] {
        let path = carts_dir().join(rel);
        if !path.exists() {
            continue;
        }
        checked += 1;
        let mut bus = graycart_gba::Bus::new(std::fs::read(&path).unwrap());
        assert!(bus.has_rtc(), "{rel}");
        bus.write16(0x0800_00C8, 1);
        bus.write16(0x0800_00C6, 0xF);
        bus.write16(0x0800_00C4, 0x1);
        bus.write16(0x0800_00C4, 0x5);
        for i in 0..8 {
            let bit = (0x63u16 >> (7 - i)) & 1;
            bus.write16(0x0800_00C4, (bit << 1) | 4);
            bus.write16(0x0800_00C4, (bit << 1) | 1 | 4);
        }
        bus.write16(0x0800_00C6, 0x5);
        let mut status = 0u8;
        for _ in 0..8 {
            bus.write16(0x0800_00C4, 0x4);
            let sio = (bus.read16(0x0800_00C4) >> 1) & 1;
            status = (status >> 1) | ((sio as u8) << 7);
            bus.write16(0x0800_00C4, 0x5);
        }
        assert_eq!(status & 0x80, 0, "{rel} POWER clear, status={status:#x}");
        assert_ne!(status & 0x40, 0, "{rel} 24h, status={status:#x}");
        println!("{rel}: RTC status={status:#x} OK");
    }
    assert!(checked >= 1, "expected at least one Pokemon ROM in carts/");
}
