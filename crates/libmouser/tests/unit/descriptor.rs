//! Unit tests for `descriptor`.
//!
//! Compiled as a child module of `crate::descriptor` via `#[path]`, so the
//! private `walk` and `Item` are in scope without being `pub`.
//!
//! ## Where the vectors come from
//!
//! Captured from `device/report_descriptor` in sysfs on a wired Logitech G502
//! HERO and a Zelotes C-18 — five interfaces between them. The expected page
//! and report-ID sets were produced by an independent walker written in
//! Python, not by this module, so they check the Rust against another
//! implementation rather than against itself.
//!
//! The short hand-decoded items below are checked against the HID 1.11 item
//! format directly, which is the real oracle: `06 00 ff` is a Global item,
//! tag 0, two little-endian data bytes, value `0xff00`.

use super::*;

// ---------------------------------------------------------------- vectors ---

/// Zelotes C-18 interface 2 — the configuration channel. 32 bytes.
const C18_CONFIG: [u8; 32] = [
    0x06, 0x00, 0xff, 0x0a, 0x00, 0xff, 0xa1, 0x01, 0x15, 0x00, 0x26, 0xff, 0x00, 0x09, 0x20,
    0x75, 0x08, 0x95, 0x40, 0x81, 0x02, 0x09, 0x21, 0x91, 0x02, 0x09, 0x22, 0x95, 0x08, 0xb1,
    0x02, 0xc0,
];

/// Zelotes C-18 interface 1 — keyboard, consumer, system, and a vendor
/// collection on `0xFF01`. The near-miss that defeats a range check.
const C18_KEYBOARD: [u8; 117] = [
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15,
    0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x19, 0x00, 0x2a, 0xff, 0x00, 0x15,
    0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x0e, 0x81, 0x00, 0xc0, 0x05, 0x01, 0x09, 0x80,
    0xa1, 0x01, 0x85, 0x02, 0x19, 0x81, 0x29, 0x83, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95,
    0x03, 0x81, 0x02, 0x95, 0x05, 0x81, 0x01, 0xc0, 0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85,
    0x03, 0x19, 0x00, 0x2a, 0xff, 0x02, 0x15, 0x00, 0x26, 0xff, 0x02, 0x75, 0x10, 0x95, 0x01,
    0x81, 0x00, 0xc0, 0x06, 0x01, 0xff, 0x0a, 0x01, 0xff, 0xa1, 0x01, 0x85, 0x06, 0x15, 0x00,
    0x26, 0xff, 0x00, 0x09, 0x2f, 0x75, 0x08, 0x95, 0x03, 0x81, 0x00, 0xc0,
];

/// Zelotes C-18 interface 0 — plain boot mouse, no report IDs. 71 bytes.
const C18_MOUSE: [u8; 71] = [
    0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x15, 0x00, 0x25,
    0x01, 0x19, 0x01, 0x29, 0x05, 0x75, 0x01, 0x95, 0x05, 0x81, 0x02, 0x95, 0x03, 0x81, 0x01,
    0x05, 0x01, 0x16, 0x01, 0x80, 0x26, 0xff, 0x7f, 0x09, 0x30, 0x09, 0x31, 0x75, 0x10, 0x95,
    0x02, 0x81, 0x06, 0x15, 0x81, 0x25, 0x7f, 0x09, 0x38, 0x75, 0x08, 0x95, 0x01, 0x81, 0x06,
    0x05, 0x0c, 0x0a, 0x38, 0x02, 0x95, 0x01, 0x81, 0x06, 0xc0, 0xc0,
];

/// Logitech G502 HERO interface 1 — the HID++ node. 151 bytes, five report
/// IDs, and `0xFF00` *plus* report `0x10`.
const G502_HIDPP: [u8; 151] = [
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15,
    0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x81, 0x03, 0x95, 0x06, 0x75, 0x08,
    0x15, 0x00, 0x26, 0xa4, 0x00, 0x19, 0x00, 0x2a, 0xa4, 0x00, 0x81, 0x00, 0xc0, 0x05, 0x0c,
    0x09, 0x01, 0xa1, 0x01, 0x85, 0x03, 0x75, 0x10, 0x95, 0x02, 0x15, 0x01, 0x26, 0x8c, 0x02,
    0x19, 0x01, 0x2a, 0x8c, 0x02, 0x81, 0x00, 0xc0, 0x05, 0x01, 0x09, 0x80, 0xa1, 0x01, 0x85,
    0x04, 0x75, 0x02, 0x95, 0x01, 0x15, 0x01, 0x25, 0x03, 0x09, 0x82, 0x09, 0x81, 0x09, 0x83,
    0x81, 0x60, 0x75, 0x06, 0x81, 0x03, 0xc0, 0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, 0x85,
    0x10, 0x75, 0x08, 0x95, 0x06, 0x15, 0x00, 0x26, 0xff, 0x00, 0x09, 0x01, 0x81, 0x00, 0x09,
    0x01, 0x91, 0x00, 0xc0, 0x06, 0x00, 0xff, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x11, 0x75, 0x08,
    0x95, 0x13, 0x15, 0x00, 0x26, 0xff, 0x00, 0x09, 0x02, 0x81, 0x00, 0x09, 0x02, 0x91, 0x00,
    0xc0,
];

/// Logitech G502 HERO interface 0 — boot mouse. 67 bytes.
const G502_MOUSE: [u8; 67] = [
    0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29,
    0x10, 0x15, 0x00, 0x25, 0x01, 0x95, 0x10, 0x75, 0x01, 0x81, 0x02, 0x05, 0x01, 0x16, 0x01,
    0x80, 0x26, 0xff, 0x7f, 0x75, 0x10, 0x95, 0x02, 0x09, 0x30, 0x09, 0x31, 0x81, 0x06, 0x15,
    0x81, 0x25, 0x7f, 0x75, 0x08, 0x95, 0x01, 0x09, 0x38, 0x81, 0x06, 0x05, 0x0c, 0x0a, 0x38,
    0x02, 0x95, 0x01, 0x81, 0x06, 0xc0, 0xc0,
];

/// Every captured interface with what an independent walker found in it.
fn captured() -> Vec<(&'static str, &'static [u8], Vec<u16>, Vec<u8>)> {
    vec![
        ("C-18 iface 0", &C18_MOUSE, vec![0x01, 0x09, 0x0c], vec![]),
        (
            "C-18 iface 1",
            &C18_KEYBOARD,
            vec![0x01, 0x07, 0x0c, 0xff01],
            vec![0x01, 0x02, 0x03, 0x06],
        ),
        ("C-18 iface 2", &C18_CONFIG, vec![0xff00], vec![]),
        ("G502 iface 0", &G502_MOUSE, vec![0x01, 0x09, 0x0c], vec![]),
        (
            "G502 iface 1",
            &G502_HIDPP,
            vec![0x01, 0x07, 0x0c, 0xff00],
            vec![0x01, 0x03, 0x04, 0x10, 0x11],
        ),
    ]
}

// ------------------------------------------------------------------- walk ---

#[test]
fn an_empty_descriptor_walks_to_no_items() {
    assert_eq!(walk(&[]).unwrap(), Vec::new());
    assert!(ReportDescriptor::parse(&[]).unwrap().usage_pages().is_empty());
}

#[test]
fn the_config_descriptor_walks_to_its_hand_decoded_items() {
    // Decoded against the HID 1.11 item format by hand — the real oracle for
    // the walker, since it does not depend on any implementation.
    //
    //   06 00 ff   Global tag 0 size 2  Usage Page 0xff00
    //   0a 00 ff   Local  tag 0 size 2  Usage 0xff00
    //   a1 01      Main   tag a size 1  Collection (Application)
    //   15 00      Global tag 1 size 1  Logical Minimum 0
    //   26 ff 00   Global tag 2 size 2  Logical Maximum 255
    //   09 20      Local  tag 0 size 1  Usage 0x20
    //   75 08      Global tag 7 size 1  Report Size 8
    //   95 40      Global tag 9 size 1  Report Count 64
    //   81 02      Main   tag 8 size 1  Input
    //   09 21      Local  tag 0 size 1  Usage 0x21
    //   91 02      Main   tag 9 size 1  Output
    //   09 22      Local  tag 0 size 1  Usage 0x22
    //   95 08      Global tag 9 size 1  Report Count 8
    //   b1 02      Main   tag b size 1  Feature
    //   c0         Main   tag c size 0  End Collection
    let items = walk(&C18_CONFIG).expect("the C-18 config descriptor must walk");

    let expected: &[(u8, u8, u32)] = &[
        (1, 0x0, 0xff00),
        (2, 0x0, 0xff00),
        (0, 0xa, 0x01),
        (1, 0x1, 0x00),
        (1, 0x2, 0xff),
        (2, 0x0, 0x20),
        (1, 0x7, 0x08),
        (1, 0x9, 0x40),
        (0, 0x8, 0x02),
        (2, 0x0, 0x21),
        (0, 0x9, 0x02),
        (2, 0x0, 0x22),
        (1, 0x9, 0x08),
        (0, 0xb, 0x02),
        (0, 0xc, 0x00),
    ];

    assert_eq!(items.len(), expected.len(), "got {items:#?}");

    for (got, &(item_type, tag, value)) in items.iter().zip(expected) {
        assert_eq!(got.item_type, item_type, "{got:?}");
        assert_eq!(got.tag, tag, "{got:?}");
        assert_eq!(got.value, value, "{got:?}");
    }
}

#[test]
fn multi_byte_values_are_little_endian() {
    // `26 ff 7f` is Logical Maximum 32767, not 65407. Getting the byte order
    // backwards leaves the Usage Page tests passing by luck — 0xff00 reversed
    // is 0x00ff, which is wrong but still not a page any device here uses, so
    // only a value with two distinct non-zero bytes catches it.
    let items = walk(&[0x26, 0xff, 0x7f]).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].value, 0x7fff);
}

#[test]
fn a_size_field_of_three_means_four_data_bytes() {
    // The classic desync. `bSize` 3 is four bytes, not three. A walker that
    // skips three reads the fourth as the next prefix and everything after is
    // nonsense — so this checks both the value and that the walk ends cleanly
    // with exactly two items rather than three.
    let items = walk(&[0x27, 0x01, 0x02, 0x03, 0x04, 0xc0]).unwrap();

    assert_eq!(items.len(), 2, "got {items:#?}");
    assert_eq!(items[0].value, 0x0403_0201);
    assert_eq!(items[1].tag, 0xc, "End Collection should follow");
}

#[test]
fn a_long_item_is_skipped_without_desyncing() {
    // `FE <len> <tag> <len bytes>`. Vanishingly rare, but a walker that does
    // not recognise 0xFE reads it as a Reserved item with two data bytes and
    // every following offset is wrong. The End Collection after it is what
    // proves the walk stayed aligned.
    let items = walk(&[0xfe, 0x03, 0x42, 0xaa, 0xbb, 0xcc, 0xc0]).unwrap();

    assert_eq!(items.len(), 2, "got {items:#?}");
    assert_eq!(items[1].tag, 0xc, "End Collection should follow the long item");
}

#[test]
fn a_truncated_item_is_an_error_not_a_panic() {
    // These bytes come from the kernel, so a malformed descriptor has to be
    // an error. Slicing past the end would panic inside the daemon.
    for bytes in [
        vec![0x06],                               // claims 2 bytes, none follow
        vec![0x06, 0x00],                         // claims 2, one follows
        vec![0x27, 0x01, 0x02],                   // claims 4, two follow
        vec![0xfe],                               // long item, no length
        vec![0xfe, 0x04],                         // long item, no tag
        vec![0xfe, 0x04, 0x42, 0xaa],             // long item short by 3
        vec![0x05, 0x01, 0x06, 0x00],             // valid item then truncated
    ] {
        assert!(
            walk(&bytes).is_err(),
            "{bytes:02x?} is truncated and must be rejected"
        );
        assert!(
            ReportDescriptor::parse(&bytes).is_err(),
            "{bytes:02x?} should fail the parse too"
        );
    }
}

#[test]
fn a_zero_size_item_consumes_only_its_prefix() {
    // `c0` is End Collection with no data. Three of them in a row is three
    // items; a walker that always consumed a data byte would see one.
    let items = walk(&[0xc0, 0xc0, 0xc0]).unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|item| item.value == 0));
}

// ----------------------------------------------------------------- parse ---

#[test]
fn every_captured_interface_yields_what_an_independent_walker_found() {
    for (name, bytes, pages, ids) in captured() {
        let parsed = ReportDescriptor::parse(bytes)
            .unwrap_or_else(|err| panic!("{name} failed to parse: {err}"));

        assert_eq!(parsed.usage_pages(), pages, "{name}: usage pages");
        assert_eq!(parsed.report_ids(), ids, "{name}: report ids");
    }
}

#[test]
fn usage_pages_keep_declaration_order_and_are_deduplicated() {
    // The G502's HID++ node declares 0x01, then 0x0c, then 0x01 again, then
    // 0xff00 twice. Order is kept because "the first page declared" is a
    // question drivers ask; duplicates are dropped.
    let parsed = ReportDescriptor::parse(&G502_HIDPP).unwrap();

    assert_eq!(parsed.usage_pages(), [0x01, 0x07, 0x0c, 0xff00]);
    assert_eq!(parsed.usage_pages()[0], 0x01, "Generic Desktop comes first");
}

#[test]
fn report_ids_are_ascending_and_deduplicated() {
    let parsed = ReportDescriptor::parse(&G502_HIDPP).unwrap();
    let ids = parsed.report_ids();

    assert_eq!(ids, [0x01, 0x03, 0x04, 0x10, 0x11]);
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ascending, no repeats");
}

#[test]
fn a_one_byte_usage_page_is_zero_extended() {
    // `05 01` is one data byte; the page is 0x0001, not 0x0100.
    let parsed = ReportDescriptor::parse(&[0x05, 0x01]).unwrap();
    assert_eq!(parsed.usage_pages(), [0x0001]);
}

#[test]
fn local_and_main_items_are_not_mistaken_for_global_ones() {
    // `09 20` is a *Local* Usage with tag 0 — the same tag as a Global Usage
    // Page. Keying on the tag without checking the type would record 0x20 as
    // a usage page. Likewise `85` as a Main tag is not a Report ID.
    let parsed = ReportDescriptor::parse(&C18_CONFIG).unwrap();

    assert_eq!(
        parsed.usage_pages(),
        [0xff00],
        "the three Local usages (0x20, 0x21, 0x22) must not appear here"
    );
    assert!(parsed.report_ids().is_empty());
}

// ------------------------------------------------------------ predicates ---

#[test]
fn the_c18_config_interface_is_the_only_one_matching_its_predicate() {
    // usage page 0xFF00 and no report IDs. The G502's HID++ node also has
    // 0xFF00, so the empty report-ID set is the discriminator.
    for (name, bytes, _, _) in captured() {
        let parsed = ReportDescriptor::parse(bytes).unwrap();
        let matches = parsed.has_usage_page(0xff00) && parsed.is_unnumbered();

        assert_eq!(
            matches,
            name == "C-18 iface 2",
            "{name}: pages {:04x?} reports {:02x?}",
            parsed.usage_pages(),
            parsed.report_ids()
        );
    }
}

#[test]
fn the_g502_hidpp_node_is_the_only_one_matching_its_predicate() {
    for (name, bytes, _, _) in captured() {
        let parsed = ReportDescriptor::parse(bytes).unwrap();
        let matches = parsed.has_usage_page(0xff00) && parsed.has_report(0x10);

        assert_eq!(matches, name == "G502 iface 1", "{name}");
    }
}

#[test]
fn a_vendor_page_range_check_would_be_wrong() {
    // Documents why `has_usage_page` is an exact match. The C-18's keyboard
    // interface uses 0xFF01, so "any page in 0xFF00..=0xFFFF" matches both it
    // and the config interface — and then the driver writes 64-byte payloads
    // to a keyboard.
    let config = ReportDescriptor::parse(&C18_CONFIG).unwrap();
    let keyboard = ReportDescriptor::parse(&C18_KEYBOARD).unwrap();

    assert!(config.has_usage_page(0xff00));
    assert!(!keyboard.has_usage_page(0xff00), "it is 0xff01, one off");

    let loose = |d: &ReportDescriptor| d.usage_pages().iter().any(|p| (0xff00..=0xffff).contains(p));
    assert!(loose(&config) && loose(&keyboard), "both match the loose test");
}

#[test]
fn an_unnumbered_descriptor_is_recognised() {
    for (name, bytes, _, ids) in captured() {
        let parsed = ReportDescriptor::parse(bytes).unwrap();
        assert_eq!(parsed.is_unnumbered(), ids.is_empty(), "{name}");
    }
}
