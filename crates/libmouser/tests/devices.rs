//! Integration tests: the shipped `.device` database, through the public API.
//!
//! This file compiles as its own crate and links `libmouser` the way any
//! consumer would, so it can only reach `pub` items. It validates the real
//! data in `devices/` rather than fixtures — if a shipped file is malformed,
//! or two entries claim the same hardware, this is what catches it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use libmouser::db::DeviceMatch;
use libmouser::{BusType, DeviceDb, DeviceType};

/// The repository's `devices/` directory, two levels up from this crate.
fn devices_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../devices")
}

fn shipped_db() -> DeviceDb {
    DeviceDb::load_dir(devices_dir()).expect("the shipped database should load")
}

#[test]
fn every_shipped_file_parses() {
    let dir = devices_dir();
    let mut seen = 0;

    for dirent in fs::read_dir(&dir).expect("devices/ should exist") {
        let path = dirent.expect("readable entry").path();
        if path.extension().is_none_or(|ext| ext != "device") {
            continue;
        }

        let entry = DeviceDb::parse_file(&path)
            .unwrap_or_else(|err| panic!("{} failed to parse: {err}", path.display()));

        assert!(!entry.name.is_empty(), "{} has an empty Name", path.display());
        assert!(
            !entry.matches.is_empty(),
            "{} matches no hardware",
            path.display()
        );
        assert!(
            !entry.driver.is_empty(),
            "{} names no driver",
            path.display()
        );
        seen += 1;
    }

    assert!(seen > 0, "no .device files found in {}", dir.display());
}

#[test]
fn load_dir_finds_every_file_and_sorts_by_name() {
    let db = shipped_db();
    let names: Vec<&str> = db.entries().iter().map(|e| e.name.as_str()).collect();

    assert!(!db.is_empty());

    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "load_dir should return entries name-sorted");

    // Every file in devices/ should have produced exactly one entry.
    let file_count = fs::read_dir(devices_dir())
        .expect("devices/ should exist")
        .filter_map(Result::ok)
        .filter(|d| d.path().extension().is_some_and(|ext| ext == "device"))
        .count();
    assert_eq!(db.entries().len(), file_count);
}

/// Two entries claiming the same bus/vid/pid would make `find` depend on
/// directory order, which is a database bug rather than a code one.
#[test]
fn no_two_entries_claim_the_same_hardware() {
    let db = shipped_db();
    let mut owner: HashMap<DeviceMatch, &str> = HashMap::new();

    for entry in db.entries() {
        for m in &entry.matches {
            if let Some(previous) = owner.insert(*m, &entry.name) {
                panic!(
                    "{:?} {:04x}:{:04x} is claimed by both {:?} and {:?}",
                    m.bus, m.vendor, m.product, previous, entry.name
                );
            }
        }
    }
}

#[test]
fn every_shipped_device_is_a_mouse_on_the_hidpp20_driver() {
    // True today because the whole shipped set is Logitech. When the first
    // non-Logitech device lands, this test is meant to fail and be narrowed.
    for entry in shipped_db().entries() {
        assert_eq!(entry.device_type, DeviceType::Mouse, "{}", entry.name);
        assert_eq!(entry.driver, "hidpp20", "{}", entry.name);
    }
}

#[test]
fn known_hardware_resolves_to_the_right_entry() {
    let db = shipped_db();

    let cases = [
        (BusType::Usb, 0x046du16, 0xc08bu16, "Logitech G502 Hero"),
        (BusType::Bluetooth, 0x046d, 0xb034, "Logitech MX Master 3S"),
        (BusType::Bluetooth, 0x046d, 0xb023, "Logitech MX Master 3"),
        (BusType::Bluetooth, 0x046d, 0xb019, "Logitech MX Master 2S"),
        (BusType::Usb, 0x046d, 0x4069, "Logitech MX Master 2S"),
        (BusType::Bluetooth, 0x046d, 0xb012, "Logitech MX Master"),
        (BusType::Usb, 0x046d, 0x4041, "Logitech MX Master"),
        (BusType::Usb, 0x046d, 0xc08a, "Logitech MX Vertical"),
        (BusType::Bluetooth, 0x046d, 0xb020, "Logitech MX Vertical"),
    ];

    for (bus, vendor, product, want) in cases {
        let found = db
            .find(bus, vendor, product)
            .unwrap_or_else(|| panic!("{bus:?} {vendor:04x}:{product:04x} should match something"));
        assert_eq!(found.name, want);
    }
}

#[test]
fn unknown_hardware_does_not_match() {
    let db = shipped_db();
    // A Logitech receiver, not a device we configure.
    assert!(db.find(BusType::Usb, 0x046d, 0x0000).is_none());
    // Not Logitech at all.
    assert!(db.find(BusType::Usb, 0x1038, 0x1702).is_none());
}

/// A malformed file must be skipped rather than taking the whole load down —
/// one bad file in a packaged database should not stop the daemon starting.
#[test]
fn load_dir_skips_junk_and_unparseable_files() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("db-skip");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");

    fs::write(
        dir.join("good.device"),
        "[Device]\nName=Good\nDeviceMatch=usb:046d:c08b\nDeviceType=mouse\nDriver=hidpp20\n",
    )
    .unwrap();

    // Missing every required key.
    fs::write(dir.join("broken.device"), "[Device]\nName=Broken\n").unwrap();

    // Right shape, wrong extension — should not be read at all.
    fs::write(
        dir.join("README.txt"),
        "[Device]\nName=Ignored\nDeviceMatch=usb:1234:5678\nDeviceType=mouse\nDriver=hidpp20\n",
    )
    .unwrap();

    let db = DeviceDb::load_dir(&dir).expect("a bad file must not fail the load");

    let names: Vec<&str> = db.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["Good"]);
}

#[test]
fn load_dir_on_a_missing_directory_is_an_error() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("does-not-exist");
    assert!(DeviceDb::load_dir(&missing).is_err());
}

#[test]
fn load_dir_on_an_empty_directory_yields_an_empty_db() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("db-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");

    let db = DeviceDb::load_dir(&dir).expect("an empty directory is not an error");
    assert!(db.is_empty());
}
