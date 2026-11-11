//! Unit tests for `db`.
//!
//! Compiled as a child module of `crate::db` via `#[path]`, which is what
//! gives these tests access to the private `parse_entry`.

use super::*;

/// A fixture covering everything the parser has to handle: comments in both
/// styles, a multi-token `DeviceMatch`, a driver section, and a nested
/// sub-section.
const FULL: &str = "\
# a hash comment
; a semicolon comment

[Device]
Name=Test Mouse
DeviceMatch=usb:046d:c08b;bluetooth:046d:b034
DeviceType=mouse
Driver=testdrv

[Driver/testdrv]
Profiles=3
DpiRange=100:16000@100
DpiList=1600;400;800;400
Wireless=1
Quirks=DOUBLE_DPI;STRIX_PROFILE
Quirk=EXTRA

[Driver/testdrv/devices/V102]
Buttons=7
";

fn parse(text: &str) -> Result<DeviceEntry> {
    parse_entry(Path::new("test.device"), text)
}

fn full() -> DeviceEntry {
    parse(FULL).expect("the fixture should parse")
}

// ---- DeviceMatch ----

#[test]
fn device_match_parses_a_well_formed_token() {
    assert_eq!(
        DeviceMatch::parse("usb:046d:c08b"),
        Some(DeviceMatch {
            bus: BusType::Usb,
            vendor: 0x046d,
            product: 0xc08b,
        })
    );
    assert_eq!(
        DeviceMatch::parse("bluetooth:046d:b034"),
        Some(DeviceMatch {
            bus: BusType::Bluetooth,
            vendor: 0x046d,
            product: 0xb034,
        })
    );
}

#[test]
fn device_match_rejects_malformed_tokens() {
    for token in [
        "",
        "usb",
        "usb:046d",
        "usb:046d:c08b:0",   // trailing field
        "usb:046d:c08b:",    // trailing separator
        "usb::c08b",         // empty vendor
        "usb:046d:zzzz",     // not hex
        "usb:0x046d:c08b",   // no 0x prefix allowed
        "usb:46d:c08b:extra",
    ] {
        assert!(
            DeviceMatch::parse(token).is_none(),
            "{token:?} should not parse"
        );
    }
}

/// An unrecognised bus token degrades to `Unknown` rather than failing the
/// parse, so such an entry loads but can only ever be reached by the
/// bus-agnostic fallback in `find`.
#[test]
fn device_match_degrades_an_unknown_bus() {
    let parsed = DeviceMatch::parse("i2c:046d:c08b").expect("still parses");
    assert_eq!(parsed.bus, BusType::Unknown);
    assert_eq!(parsed.vendor, 0x046d);
}

/// Hex digits are accepted in either case even though every shipped file uses
/// lowercase. Worth pinning: it means `find` cannot rely on case to
/// distinguish entries.
#[test]
fn device_match_accepts_uppercase_hex() {
    assert_eq!(
        DeviceMatch::parse("usb:046D:C08B"),
        DeviceMatch::parse("usb:046d:c08b")
    );
}

// ---- DpiRange ----

#[test]
fn dpi_range_parses_min_max_step() {
    assert_eq!(
        DpiRange::parse("100:16000@100"),
        Some(DpiRange {
            min: 100,
            max: 16000,
            step: 100,
        })
    );
    assert_eq!(
        DpiRange::parse(" 200 : 800 @ 50 "),
        Some(DpiRange {
            min: 200,
            max: 800,
            step: 50,
        }),
        "whitespace around the numbers is tolerated"
    );
}

#[test]
fn dpi_range_rejects_nonsense() {
    for s in [
        "",
        "100:16000",     // no step
        "100@100",       // no max
        "16000:100@100", // min > max
        "100:16000@0",   // zero step would loop forever
        "a:b@c",
        "100:16000@100@100",
    ] {
        assert!(DpiRange::parse(s).is_none(), "{s:?} should not parse");
    }
}

#[test]
fn dpi_range_expands_to_every_step() {
    let range = DpiRange::parse("100:300@100").unwrap();
    assert_eq!(range.values(), vec![100, 200, 300]);

    // A span that is not a whole number of steps stops before overshooting.
    let ragged = DpiRange::parse("100:250@100").unwrap();
    assert_eq!(ragged.values(), vec![100, 200]);

    // min == max is a single legal value.
    let single = DpiRange::parse("800:800@100").unwrap();
    assert_eq!(single.values(), vec![800]);
}

// ---- parse_entry: the happy path ----

#[test]
fn parse_entry_reads_the_device_section() {
    let entry = full();

    assert_eq!(entry.name, "Test Mouse");
    assert_eq!(entry.driver, "testdrv");
    assert_eq!(entry.device_type, DeviceType::Mouse);
    assert_eq!(entry.path, Path::new("test.device"));

    assert_eq!(
        entry.matches,
        vec![
            DeviceMatch {
                bus: BusType::Usb,
                vendor: 0x046d,
                product: 0xc08b,
            },
            DeviceMatch {
                bus: BusType::Bluetooth,
                vendor: 0x046d,
                product: 0xb034,
            },
        ],
        "matches keep their file order"
    );
}

#[test]
fn parse_entry_strips_the_driver_prefix_from_section_names() {
    let entry = full();
    let sections: Vec<&str> = entry.driver_sections.keys().map(String::as_str).collect();
    assert_eq!(sections, vec!["testdrv", "testdrv/devices/V102"]);
}

#[test]
fn parse_entry_ignores_comments_and_blank_lines() {
    let minimal = "\
[Device]
Name=Bare
DeviceMatch=usb:046d:c08b
DeviceType=mouse
Driver=testdrv
";
    let entry = parse(minimal).expect("a file with no driver section is valid");
    assert_eq!(entry.name, "Bare");
    assert!(
        entry.driver_sections.is_empty(),
        "no [Driver/...] section means no private keys"
    );
    assert!(entry.driver_section().is_none());
    assert_eq!(entry.get("Profiles"), None);
}

// ---- parse_entry: the error paths ----

#[test]
fn parse_entry_rejects_an_unclosed_section_header() {
    let err = parse("[Device\nName=x\n").expect_err("missing ]");
    assert!(matches!(err, Error::Parse { line: 1, .. }), "got {err:?}");
}

#[test]
fn parse_entry_rejects_an_unknown_section() {
    let err = parse("[Nonsense]\nKey=value\n").expect_err("unknown section");
    let message = err.to_string();
    assert!(message.contains("Nonsense"), "{message}");
}

#[test]
fn parse_entry_rejects_a_key_before_any_section() {
    let err = parse("Name=orphan\n").expect_err("key with no section");
    assert!(matches!(err, Error::Parse { line: 1, .. }), "got {err:?}");
}

#[test]
fn parse_entry_rejects_a_line_that_is_neither_key_nor_section() {
    let err = parse("[Device]\njust some words\n").expect_err("not Key=Value");
    assert!(matches!(err, Error::Parse { line: 2, .. }), "got {err:?}");
}

#[test]
fn parse_entry_requires_every_device_key() {
    // Drop one required key at a time from an otherwise valid file.
    let lines = [
        "Name=Test Mouse",
        "DeviceMatch=usb:046d:c08b",
        "DeviceType=mouse",
        "Driver=testdrv",
    ];

    for omit in 0..lines.len() {
        let body: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != omit)
            .map(|(_, l)| *l)
            .collect();
        let text = format!("[Device]\n{}\n", body.join("\n"));

        assert!(
            parse(&text).is_err(),
            "omitting {:?} should fail the parse",
            lines[omit]
        );
    }
}

#[test]
fn parse_entry_rejects_a_malformed_match_token() {
    let text = "\
[Device]
Name=Test Mouse
DeviceMatch=usb:046d:c08b;garbage
DeviceType=mouse
Driver=testdrv
";
    let err = parse(text).expect_err("one bad token fails the entry");
    assert!(err.to_string().contains("garbage"), "{err}");
}

#[test]
fn parse_entry_rejects_an_empty_match_list() {
    let text = "\
[Device]
Name=Test Mouse
DeviceMatch=
DeviceType=mouse
Driver=testdrv
";
    assert!(parse(text).is_err(), "an entry matching nothing is useless");
}

#[test]
fn parse_entry_rejects_an_unknown_device_type() {
    let text = "\
[Device]
Name=Test Mouse
DeviceMatch=usb:046d:c08b
DeviceType=tablet
Driver=testdrv
";
    assert!(parse(text).is_err());
}

// ---- DeviceEntry accessors ----

#[test]
fn entry_reads_its_own_driver_section() {
    let entry = full();
    assert_eq!(entry.get("Profiles"), Some("3"));
    assert_eq!(entry.get_u32("Profiles"), Some(3));
    assert_eq!(entry.get("Missing"), None);
    assert_eq!(entry.get_u32("Quirks"), None, "not a number");
}

#[test]
fn entry_reads_a_nested_section_by_name() {
    let entry = full();
    assert_eq!(entry.get_in("testdrv/devices/V102", "Buttons"), Some("7"));
    assert_eq!(entry.get_in("testdrv", "Profiles"), Some("3"));
    assert_eq!(entry.get_in("nope", "Buttons"), None);
    assert_eq!(
        entry.get("Buttons"),
        None,
        "a nested section's keys are not visible from the top-level lookup"
    );
}

#[test]
fn entry_bools_accept_both_spellings() {
    let entry = full();
    assert_eq!(entry.get_bool("Wireless"), Some(true));
    assert_eq!(entry.get_bool("Profiles"), None, "`3` is not a bool");
    assert_eq!(entry.get_bool("Missing"), None);
}

#[test]
fn entry_lists_split_on_semicolons() {
    let entry = full();
    assert_eq!(
        entry.get_list("Quirks"),
        vec!["DOUBLE_DPI", "STRIX_PROFILE"]
    );
    assert!(
        entry.get_list("Missing").is_empty(),
        "a missing key is an empty list, not an error"
    );
}

#[test]
fn entry_dpi_list_is_sorted_and_deduped() {
    let entry = full();
    // The fixture lists 1600;400;800;400 — the API requires ascending, unique.
    assert_eq!(entry.dpi_list(), vec![400, 800, 1600]);
}

#[test]
fn entry_dpi_range_parses_from_the_driver_section() {
    let entry = full();
    assert_eq!(
        entry.dpi_range(),
        Some(DpiRange {
            min: 100,
            max: 16000,
            step: 100,
        })
    );
}

/// Some shipped files use the singular `Quirk=`, so both spellings are
/// collected rather than one winning.
#[test]
fn entry_quirks_merges_both_spellings() {
    let entry = full();
    assert_eq!(
        entry.quirks(),
        vec!["DOUBLE_DPI", "STRIX_PROFILE", "EXTRA"]
    );
}

// ---- DeviceDb::find ----

fn db_with(entries: &[&str]) -> DeviceDb {
    let mut db = DeviceDb::new();
    for text in entries {
        db.push(parse(text).expect("fixture should parse"));
    }
    db
}

fn entry_text(name: &str, matches: &str) -> String {
    format!(
        "[Device]\nName={name}\nDeviceMatch={matches}\nDeviceType=mouse\nDriver=testdrv\n"
    )
}

#[test]
fn find_prefers_an_exact_bus_match() {
    let wired = entry_text("Wired", "usb:046d:c08b");
    let wireless = entry_text("Wireless", "bluetooth:046d:c08b");
    let db = db_with(&[&wired, &wireless]);

    assert_eq!(
        db.find(BusType::Usb, 0x046d, 0xc08b).map(|e| e.name.as_str()),
        Some("Wired")
    );
    assert_eq!(
        db.find(BusType::Bluetooth, 0x046d, 0xc08b)
            .map(|e| e.name.as_str()),
        Some("Wireless")
    );
}

/// Some Bluetooth devices report their USB IDs, so a vid/pid hit on the wrong
/// bus still counts — just at lower priority than an exact match.
#[test]
fn find_falls_back_to_a_bus_agnostic_match() {
    let bt_only = entry_text("Bluetooth Only", "bluetooth:046d:b034");
    let db = db_with(&[&bt_only]);

    assert_eq!(
        db.find(BusType::Usb, 0x046d, 0xb034)
            .map(|e| e.name.as_str()),
        Some("Bluetooth Only"),
        "the bus differs but the IDs match"
    );
}

#[test]
fn find_returns_none_for_unknown_hardware() {
    let db = db_with(&[&entry_text("Only", "usb:046d:c08b")]);

    assert!(db.find(BusType::Usb, 0x1234, 0x5678).is_none());
    assert!(
        db.find(BusType::Usb, 0x046d, 0x9999).is_none(),
        "same vendor, different product"
    );
}

#[test]
fn find_searches_every_token_of_a_multi_match_entry() {
    // Mirrors devices/logitech-mx-vertical.device, which lists three IDs.
    let text = entry_text("MX Vertical", "bluetooth:046d:b020;usb:046d:407b;usb:046d:c08a");
    let db = db_with(&[&text]);

    for (bus, vendor, product) in [
        (BusType::Bluetooth, 0x046d, 0xb020),
        (BusType::Usb, 0x046d, 0x407b),
        (BusType::Usb, 0x046d, 0xc08a),
    ] {
        assert!(
            db.find(bus, vendor, product).is_some(),
            "{vendor:04x}:{product:04x} on {bus:?} should match"
        );
    }
}

#[test]
fn empty_db_matches_nothing() {
    let db = DeviceDb::new();
    assert!(db.is_empty());
    assert!(db.entries().is_empty());
    assert!(db.find(BusType::Usb, 0x046d, 0xc08b).is_none());
}
