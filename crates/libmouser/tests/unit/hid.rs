//! Unit tests for `hid`.
//!
//! Compiled as a child module of `crate::hid` via `#[path]`, so private items
//! (`ioc`, the `IOC_*` constants, `feature_ioctl`) are in scope without being
//! `pub`. Cargo only auto-discovers `tests/*.rs` and `tests/*/main.rs`, so
//! this file is never built as a standalone integration test.

use super::*;

/// The expected opcodes are an **external** oracle: they were printed by a C
/// program compiled against this machine's `<linux/hidraw.h>`, not derived
/// from `ioc`. A round-trip through our own encoder would prove nothing.
///
/// ```c
/// #include <stdio.h>
/// #include <linux/hidraw.h>
/// int main(void) {
///     printf("%08lX\n", (unsigned long)HIDIOCGRAWINFO);
///     printf("%08lX\n", (unsigned long)HIDIOCSFEATURE(8));
///     return 0;
/// }
/// ```
#[test]
fn opcodes_match_the_kernel_headers() {
    let cases: [(&str, u32, u32); 6] = [
        (
            "HIDIOCGRAWINFO",
            ioc(IOC_READ, HID, 0x03, size_of::<HidrawDevinfo>() as u32),
            0x8008_4803,
        ),
        (
            "HIDIOCGRAWNAME(256)",
            ioc(IOC_READ, HID, 0x04, 256),
            0x8100_4804,
        ),
        (
            "HIDIOCGRAWPHYS(256)",
            ioc(IOC_READ, HID, 0x05, 256),
            0x8100_4805,
        ),
        (
            "HIDIOCSFEATURE(8)",
            ioc(IOC_WRITE | IOC_READ, HID, 0x06, 8),
            0xC008_4806,
        ),
        (
            "HIDIOCGFEATURE(8)",
            ioc(IOC_WRITE | IOC_READ, HID, 0x07, 8),
            0xC008_4807,
        ),
        (
            "HIDIOCSFEATURE(20)",
            ioc(IOC_WRITE | IOC_READ, HID, 0x06, 20),
            0xC014_4806,
        ),
    ];

    for (name, got, want) in cases {
        assert_eq!(got, want, "{name}: got {got:#010x}, want {want:#010x}");
    }
}

#[test]
fn ioc_packs_each_field_into_its_own_bits() {
    // Distinct values in every field, so a transposed shift cannot pass.
    let req = ioc(IOC_READ, HID, 0x07, 20);

    assert_eq!(req >> IOC_DIRSHIFT, IOC_READ, "direction");
    assert_eq!((req >> IOC_SIZESHIFT) & 0x3fff, 20, "size");
    assert_eq!((req >> IOC_TYPESHIFT) & 0xff, HID, "type");
    assert_eq!(req & 0xff, 0x07, "nr");
}

#[test]
fn hid_is_the_ascii_letter_h() {
    assert_eq!(HID, 0x48);
}

/// `sizeof(struct hidraw_devinfo)` is 8 with 4-byte alignment: u32 + two i16,
/// no padding. If this changes, `HIDIOCGRAWINFO`'s size field is wrong and the
/// kernel will write the wrong number of bytes.
#[test]
fn devinfo_matches_the_c_struct() {
    assert_eq!(size_of::<HidrawDevinfo>(), 8);
    assert_eq!(align_of::<HidrawDevinfo>(), 4);
}

/// Why `feature_ioctl` has a length guard: the size field is 14 bits, so one
/// byte past the limit silently overflows into the direction bits and asks
/// the kernel for a completely different ioctl.
#[test]
fn oversized_length_corrupts_the_opcode() {
    assert_eq!(IOC_SIZE_MAX, 16_383);

    let ok = ioc(IOC_READ, HID, 0x04, IOC_SIZE_MAX);
    assert_eq!(ok >> IOC_DIRSHIFT, IOC_READ, "direction survives at the limit");

    let overflowed = ioc(IOC_READ, HID, 0x04, IOC_SIZE_MAX + 1);
    assert_ne!(
        overflowed >> IOC_DIRSHIFT,
        IOC_READ,
        "one byte past the limit must corrupt the direction — hence the guard"
    );
}

/// Both guards return before any ioctl is issued, so the descriptor never has
/// to be a real hidraw node.
#[test]
fn feature_ioctl_rejects_unencodable_lengths() {
    let f = File::open("/dev/null").expect("open /dev/null");
    let fd = f.as_fd();

    let mut empty: [u8; 0] = [];
    assert!(
        HidrawDevice::feature_ioctl(fd, 0x06, &mut empty).is_err(),
        "an empty report has no room for a report ID"
    );

    let mut too_big = vec![0u8; IOC_SIZE_MAX as usize + 1];
    assert!(
        HidrawDevice::feature_ioctl(fd, 0x06, &mut too_big).is_err(),
        "a length that will not fit the size field must be rejected"
    );
}

/// `/dev/null` opens fine but answers no HID ioctl, which exercises
/// `query_info`'s error path with no hardware attached.
#[test]
fn opening_a_non_hid_node_fails_cleanly() {
    let err = HidrawDevice::open("/dev/null").expect_err("/dev/null is not a hidraw node");
    assert!(
        matches!(err, Error::PlainIo(_)),
        "expected the raw errno from HIDIOCGRAWINFO, got {err:?}"
    );
}

/// The `open` itself failing is a different variant: it carries the path,
/// because at that point we know which node we were asked for.
#[test]
fn opening_a_missing_node_names_the_path() {
    let err = HidrawDevice::open("/dev/hidraw-does-not-exist").expect_err("should not exist");

    match err {
        Error::Io { path, .. } => assert_eq!(path.to_str(), Some("/dev/hidraw-does-not-exist")),
        other => panic!("expected Error::Io with a path, got {other:?}"),
    }
}
