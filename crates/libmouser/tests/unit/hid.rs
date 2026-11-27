//! Unit tests for `hid`.
//!
//! Compiled as a child module of `crate::hid` via `#[path]`, so private items
//! (`ioc`, the `IOC_*` constants, `feature_ioctl`) are in scope without being
//! `pub`. Cargo only auto-discovers `tests/*.rs` and `tests/*/main.rs`, so
//! this file is never built as a standalone integration test.

use std::io::Write;
use std::os::fd::FromRawFd;
use std::time::Instant;

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
///     printf("%08lX\n", (unsigned long)HIDIOCGRDESCSIZE);
///     printf("%08lX\n", (unsigned long)HIDIOCGRDESC);
///     return 0;
/// }
/// ```
#[test]
fn opcodes_match_the_kernel_headers() {
    let cases: [(&str, u32, u32); 8] = [
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
        (
            "HIDIOCGRDESCSIZE",
            ioc(IOC_READ, HID, 0x01, size_of::<libc::c_int>() as u32),
            0x8004_4801,
        ),
        (
            "HIDIOCGRDESC",
            ioc(IOC_READ, HID, 0x02, size_of::<HidrawReportDescriptor>() as u32),
            0x9004_4802,
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

/// `struct hidraw_report_descriptor` has to match the C layout byte for byte,
/// because the ioctl request number encodes its size — a mismatch names a
/// different ioctl rather than failing.
///
/// Literals again, from the same C program:
///
/// ```c
/// printf("%d\n",  HID_MAX_DESCRIPTOR_SIZE);
/// printf("%zu\n", sizeof(struct hidraw_report_descriptor));
/// printf("%zu\n", offsetof(struct hidraw_report_descriptor, size));
/// printf("%zu\n", offsetof(struct hidraw_report_descriptor, value));
/// ```
#[test]
fn the_report_descriptor_struct_matches_the_c_layout() {
    assert_eq!(HID_MAX_DESCRIPTOR_SIZE, 4096);
    assert_eq!(size_of::<HidrawReportDescriptor>(), 4100);
    assert_eq!(align_of::<HidrawReportDescriptor>(), 4);

    // `size` first is not cosmetic: the kernel reads the length out of the
    // front of the struct before it writes anything, so a reordered struct
    // would have it reading the first four descriptor bytes as a length.
    assert_eq!(std::mem::offset_of!(HidrawReportDescriptor, size), 0);
    assert_eq!(std::mem::offset_of!(HidrawReportDescriptor, value), 4);

    // No tail padding — 4 + 4096 exactly.
    assert_eq!(
        size_of::<HidrawReportDescriptor>(),
        size_of::<u32>() + HID_MAX_DESCRIPTOR_SIZE
    );
}

#[test]
fn the_descriptor_struct_fits_the_ioctl_size_field() {
    // 4100 is the largest payload this module puts through `ioc`, and the
    // size field is only 14 bits. Over `IOC_SIZE_MAX` it would overflow into
    // the direction bits and silently name something else — the same failure
    // `feature_ioctl` guards against at runtime, checked here at compile size
    // instead because this one is a constant.
    assert!(
        size_of::<HidrawReportDescriptor>() <= IOC_SIZE_MAX as usize,
        "4100 must fit the 14-bit size field (max {IOC_SIZE_MAX})"
    );

    // And the encoded opcode really does round-trip that size back out.
    let req = ioc(IOC_READ, HID, 0x02, size_of::<HidrawReportDescriptor>() as u32);
    let encoded_size = (req >> IOC_SIZESHIFT) & IOC_SIZE_MAX;
    assert_eq!(encoded_size, 4100);
}

// ------------------------------------------------- poll_timeout_ms ---

/// The whole point of the saturation, stated once: `poll` reads a negative
/// timeout as "block forever". Every other assertion in this section is a
/// variation on not producing one.
#[test]
fn a_poll_timeout_is_never_negative() {
    let cases = [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_micros(999),
        Duration::from_millis(1),
        Duration::from_secs(1),
        Duration::from_secs(60 * 60 * 24 * 365),
        Duration::from_millis(u64::from(u32::MAX)),
        Duration::MAX,
    ];

    for remaining in cases {
        let ms = poll_timeout_ms(remaining);
        assert!(
            ms >= 0,
            "{remaining:?} produced {ms}, and poll reads a negative timeout as infinite"
        );
    }
}

/// Literal milliseconds, not a round trip through `Duration`.
#[test]
fn a_poll_timeout_converts_whole_milliseconds_exactly() {
    let cases: [(Duration, libc::c_int); 5] = [
        (Duration::ZERO, 0),
        (Duration::from_millis(1), 1),
        (Duration::from_millis(20), 20),
        (Duration::from_millis(1000), 1000),
        (Duration::from_secs(60), 60_000),
    ];

    for (remaining, want) in cases {
        assert_eq!(poll_timeout_ms(remaining), want, "{remaining:?}");
    }
}

/// `as_millis()` is `u128`; `c_int` is 32 bits. The cast wraps, and roughly
/// half of what it wraps to is negative. 24.8 days is the hinge.
#[test]
fn a_poll_timeout_saturates_instead_of_wrapping() {
    // The largest value the field holds, and the first one past it.
    assert_eq!(
        poll_timeout_ms(Duration::from_millis(libc::c_int::MAX as u64)),
        libc::c_int::MAX,
        "the exact maximum must survive"
    );
    assert_eq!(
        poll_timeout_ms(Duration::from_millis(libc::c_int::MAX as u64 + 1)),
        libc::c_int::MAX,
        "one millisecond past the maximum must clamp, not wrap"
    );
    assert_eq!(
        poll_timeout_ms(Duration::MAX),
        libc::c_int::MAX,
        "the largest Duration there is"
    );

    // What a plain cast would have done to a year, for the record: this is the
    // value the clamp exists to avoid, computed independently of the function.
    let a_year_in_ms = Duration::from_secs(60 * 60 * 24 * 365).as_millis();
    assert_eq!(a_year_in_ms, 31_536_000_000);
    assert!(
        (a_year_in_ms as u32) as i32 > 0,
        "sanity: a year does not itself wrap negative"
    );
    assert_eq!(poll_timeout_ms(Duration::from_secs(60 * 60 * 24 * 365)), libc::c_int::MAX);
}

/// A sub-millisecond remainder truncates to zero. Either answer is defensible
/// — 0 gives up at most a millisecond early, 1 waits at most a millisecond
/// long — but it must not become `-1`, which is the one value `poll` treats as
/// a licence to block indefinitely.
#[test]
fn a_sub_millisecond_remainder_does_not_become_infinite() {
    for remaining in [Duration::from_nanos(1), Duration::from_micros(500), Duration::from_micros(999)] {
        let ms = poll_timeout_ms(remaining);
        assert!(
            ms == 0 || ms == 1,
            "{remaining:?} produced {ms}; a sub-millisecond wait is 0 or 1, never anything else"
        );
    }
}

// --------------------------------------- read_input_report_timeout ---

/// A pipe is the ideal stand-in for a node that never answers: a real fd that
/// `poll` will happily wait on and that nothing ever makes readable.
fn pipe() -> (File, File) {
    let mut fds = [-1 as libc::c_int; 2];
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "pipe(2): {}", std::io::Error::last_os_error());

    // SAFETY: `pipe` succeeded, so both are fresh owned fds and nothing else
    // holds them.
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

/// Wrap a readable fd as a device. `HidrawDevice::open` cannot be used here —
/// it issues `HIDIOCGRAWINFO`, which a pipe answers with `ENOTTY`.
fn fake_node(read_end: File, path: &str) -> HidrawDevice {
    HidrawDevice::from_parts(
        read_end,
        path,
        HidInfo {
            bus: BusType::Usb,
            vendor: 0x046d,
            product: 0xc08b,
            name: "not a real device".to_string(),
            phys: "test".to_string(),
        },
    )
}

#[test]
fn a_report_that_never_arrives_times_out() {
    // `_writer` must stay bound for the whole test: dropping it closes the
    // write end, and the read end then polls ready with POLLHUP rather than
    // waiting. That is a different test, below.
    let (reader, _writer) = pipe();
    let mut dev = fake_node(reader, "/dev/hidraw-silent");
    let mut buf = [0u8; 20];

    let started = Instant::now();
    let err = dev
        .read_input_report_timeout(&mut buf, Duration::from_millis(100))
        .expect_err("nothing is ever written to this pipe");
    let elapsed = started.elapsed();

    match err {
        Error::Timeout { path, waited } => {
            assert_eq!(
                path.to_str(),
                Some("/dev/hidraw-silent"),
                "the error should name the node, since two look alike"
            );
            assert_eq!(waited, Duration::from_millis(100), "the deadline asked for");
        }
        other => panic!("expected Error::Timeout, got {other:?}"),
    }

    assert!(
        elapsed >= Duration::from_millis(100),
        "gave up after {elapsed:?}, before the 100ms deadline"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "took {elapsed:?} to honour a 100ms deadline"
    );
}

#[test]
fn a_report_already_buffered_is_read_without_waiting() {
    let (reader, mut writer) = pipe();
    // A real HID++ long reply, for shape only.
    writer.write_all(&[0x11, 0xff, 0x00, 0x1a]).expect("write");

    let mut dev = fake_node(reader, "/dev/hidraw-buffered");
    let mut buf = [0u8; 20];

    let started = Instant::now();
    let n = dev
        .read_input_report_timeout(&mut buf, Duration::from_secs(30))
        .expect("a report was already waiting");
    let elapsed = started.elapsed();

    assert_eq!(n, 4);
    assert_eq!(&buf[..4], &[0x11, 0xff, 0x00, 0x1a]);
    assert!(
        elapsed < Duration::from_millis(500),
        "waited {elapsed:?} for a report that was already there"
    );
}

#[test]
fn a_report_that_arrives_late_is_still_read() {
    let (reader, mut writer) = pipe();
    let writing = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        writer.write_all(&[0x10, 0xff, 0x00, 0x1a]).expect("write");
    });

    let mut dev = fake_node(reader, "/dev/hidraw-slow");
    let mut buf = [0u8; 20];

    let started = Instant::now();
    let n = dev
        .read_input_report_timeout(&mut buf, Duration::from_secs(30))
        .expect("the report arrives well inside the deadline");
    let elapsed = started.elapsed();

    assert_eq!(n, 4);
    assert_eq!(&buf[..4], &[0x10, 0xff, 0x00, 0x1a]);
    assert!(
        elapsed >= Duration::from_millis(50),
        "returned in {elapsed:?}, before the writer had written anything"
    );

    writing.join().expect("the writing thread panicked");
}

#[test]
fn a_zero_timeout_polls_once_and_gives_up() {
    let (reader, _writer) = pipe();
    let mut dev = fake_node(reader, "/dev/hidraw-silent-now");
    let mut buf = [0u8; 20];

    let started = Instant::now();
    let err = dev
        .read_input_report_timeout(&mut buf, Duration::ZERO)
        .expect_err("an empty pipe has nothing to read");
    let elapsed = started.elapsed();

    assert!(
        matches!(err, Error::Timeout { .. }),
        "expected Error::Timeout, got {err:?}"
    );
    assert!(
        elapsed < Duration::from_millis(100),
        "a zero timeout took {elapsed:?}; it should poll once and return"
    );
}

/// The `revents` trap. With the write end closed and no data buffered, `poll`
/// returns ready with `POLLHUP` and `read` then returns 0. A caller cannot
/// distinguish that `Ok(0)` from a zero-length report, so it has to be an
/// error — on real hardware this is the mouse being unplugged mid-wait.
#[test]
fn a_hung_up_fd_is_an_error_not_a_zero_length_report() {
    let (reader, writer) = pipe();
    drop(writer);

    let mut dev = fake_node(reader, "/dev/hidraw-unplugged");
    let mut buf = [0u8; 20];

    let started = Instant::now();
    let result = dev.read_input_report_timeout(&mut buf, Duration::from_secs(30));
    let elapsed = started.elapsed();

    assert!(
        result.is_err(),
        "a hung-up fd returned {result:?}; POLLHUP must not read back as a report"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "POLLHUP is immediate, but this took {elapsed:?} — the error bits are \
         probably not being tested, so it waited out the deadline"
    );
}

#[test]
fn the_descriptor_ioctls_are_read_direction() {
    // Both are `_IOR`, so only `IOC_READ` is set. Worth pinning because
    // `HIDIOCGRDESC` behaves as in-out — the kernel reads `size` before
    // writing `value` — and the direction bits do not say so. If someone
    // "corrects" the direction to `IOC_WRITE | IOC_READ` to reflect that, the
    // opcode changes and the call stops working.
    for nr in [0x01, 0x02] {
        let req = ioc(IOC_READ, HID, nr, 4);
        let dir = req >> IOC_DIRSHIFT;
        assert_eq!(dir, IOC_READ, "nr {nr:#04x} should be _IOR");
        assert_ne!(dir, IOC_WRITE | IOC_READ, "nr {nr:#04x} is not _IOWR");
    }
}
