//! Unit tests for `driver::c18`.
//!
//! Compiled as a child module of `crate::driver::c18` via `#[path]`, so the
//! private helpers (`checksum`, `dpi_to_byte`, `dpi_from_byte`,
//! `polling_interval`, `wire_profile`) are in scope without being `pub`.
//! Cargo only auto-discovers `tests/*.rs` and `tests/*/main.rs`, so this file
//! is never built as a standalone integration test.
//!
//! Everything here is pure byte and table work — no hardware, no node. The
//! parts that do touch the device (`C18::send_command`, `write_payload`,
//! `probe`, `commit`) are not covered: they need a C18 on the bus, and belong
//! in `tests/hardware.rs` once they exist.
//!
//! ## Where the vectors come from
//!
//! `PROTOCOL.md` in <https://github.com/PadreAdamo/c18_reverseengineer>,
//! captured from the vendor software with USBPcap, and the calibration table
//! in libratbag PR #1877. The `begin` command below (`01 88 00 00 00 00 00
//! 76`) is quoted verbatim in both, which makes it a genuine external oracle:
//! if `Command::to_bytes` disagrees, the encoder is wrong.
//!
//! The interpolated DPI values were computed by running the upstream
//! algorithm over the upstream table, so they check *this* implementation
//! against *that* one rather than against itself.

use super::*;

// -------------------------------------------------------------- constants ---

#[test]
fn the_capabilities_are_the_ones_upstream_found() {
    // Literals on purpose: these describe hardware, and a change to one is a
    // claim about the device, not a refactor.
    assert_eq!(NUM_PROFILES, 2);
    assert_eq!(NUM_BUTTONS, 11);
    assert_eq!(NUM_RESOLUTIONS, 5);
    assert_eq!(NUM_LEDS, 1);
    assert_eq!(MAX_MACRO_KEYS, 96);
    assert_eq!(REPORT_RATES, [125, 250, 500, 1000]);
}

#[test]
fn the_wire_sizes_match_the_report_descriptor() {
    // Interface 2 declares a 64-byte In/Out pair and an 8-byte Feature
    // report. The command length is that feature report; the payload length
    // is the output report.
    assert_eq!(COMMAND_LEN, 8);
    assert_eq!(PAYLOAD_LEN, 64);
}

#[test]
fn the_opcodes_are_the_documented_ones() {
    assert_eq!(OP_BEGIN, 0x01);
    assert_eq!(OP_SELECT_COMMIT, 0x02);
    assert_eq!(OP_SET_POLLING, 0x03);
    assert_eq!(OP_SET_DPI_STAGE, 0x04);
    assert_eq!(OP_PREPARE_TABLE, 0x0c);
    assert_eq!(OP_PREPARE_BUTTON_TABLE, 0x0d);
    assert_eq!(OP_PREPARE_EXT_DATA, 0x0f);
    assert_eq!(BEGIN_ARG, 0x88);
    assert_eq!(PREPARE_ARG, 0x80);
    assert_eq!(MACRO_ID_BASE, 0x10);
}

#[test]
fn the_dpi_calibration_table_is_ascending_in_both_columns() {
    // Interpolation needs the table sorted by dpi, and a non-monotonic byte
    // column would mean a dpi increase could lower the sensor setting.
    for pair in DPI_CALIBRATION.windows(2) {
        let ((d0, b0), (d1, b1)) = (pair[0], pair[1]);
        assert!(d0 < d1, "dpi {d0} is not below {d1}");
        assert!(b0 < b1, "byte {b0:#04x} is not below {b1:#04x}");
    }
}

#[test]
fn the_dpi_bounds_are_the_ends_of_the_calibration_table() {
    assert_eq!(DPI_CALIBRATION[0].0, DPI_MIN);
    assert_eq!(DPI_CALIBRATION[DPI_CALIBRATION.len() - 1].0, DPI_MAX);
    assert_eq!(DPI_MIN, 200);
    assert_eq!(DPI_MAX, 10_000);
    assert_eq!(DPI_STEP, 100);
}

#[test]
fn every_dpi_stage_has_its_own_pair_of_table_offsets() {
    // A duplicated offset would make two stages overwrite each other.
    let mut seen = Vec::new();
    for (x, y) in DPI_STAGE_OFFSETS {
        assert!(x < PAYLOAD_LEN, "x offset {x} is past the payload");
        assert!(y < PAYLOAD_LEN, "y offset {y} is past the payload");
        assert_ne!(x, y, "x and y share offset {x}");
        seen.push(x);
        seen.push(y);
    }
    let unique = {
        let mut v = seen.clone();
        v.sort_unstable();
        v.dedup();
        v
    };
    assert_eq!(seen.len(), unique.len(), "offsets collide: {seen:?}");
}

// --------------------------------------------------------------- checksum ---

#[test]
fn the_checksum_makes_the_whole_command_sum_to_ff() {
    // The defining property, and the one you can check by eye in a hex dump.
    for command in [
        Command::begin(),
        Command::new(OP_SELECT_COMMIT, 0x01, 0x00),
        Command::new(OP_SET_POLLING, 0x02, 0x08),
        Command::new(OP_PREPARE_EXT_DATA, MACRO_ID_BASE, PREPARE_ARG),
    ] {
        let bytes = command.to_bytes();
        let sum: u32 = bytes.iter().map(|&b| u32::from(b)).sum();

        assert_eq!(
            sum % 0x100,
            0xff,
            "{bytes:02x?} sums to {sum:#x}, not 0xff mod 256"
        );
    }
}

#[test]
fn the_begin_command_is_the_documented_byte_string() {
    // Quoted verbatim in PROTOCOL.md and in upstream's driver. The load
    // bearing test in this file.
    assert_eq!(
        Command::begin().to_bytes(),
        [0x01, 0x88, 0x00, 0x00, 0x00, 0x00, 0x00, 0x76]
    );
}

#[test]
fn commands_encode_the_opcode_and_both_arguments_then_pad() {
    let bytes = Command::new(OP_SET_DPI_STAGE, 0x01, 0x05).to_bytes();

    assert_eq!(bytes[0], OP_SET_DPI_STAGE, "byte 0: opcode");
    assert_eq!(bytes[1], 0x01, "byte 1: profile");
    assert_eq!(bytes[2], 0x05, "byte 2: stage");
    assert_eq!(&bytes[3..7], &[0, 0, 0, 0], "bytes 3..7 are reserved zero");
    assert_eq!(bytes[7], 0xf5, "byte 7: checksum");
}

#[test]
fn every_documented_command_checksums_to_its_captured_value() {
    // Computed by running `0xFF - sum(bytes[0..7])` over each documented
    // command form, which is the formula PROTOCOL.md states and upstream
    // implements.
    for (opcode, b1, b2, want) in [
        (OP_BEGIN, 0x88, 0x00, 0x76),
        (OP_SELECT_COMMIT, 0x01, 0x00, 0xfc),
        (OP_SELECT_COMMIT, 0x02, 0x00, 0xfb),
        (OP_SET_POLLING, 0x01, 0x01, 0xfa),
        (OP_SET_DPI_STAGE, 0x01, 0x05, 0xf5),
        (OP_PREPARE_TABLE, 0x01, PREPARE_ARG, 0x72),
        (OP_PREPARE_BUTTON_TABLE, 0x02, PREPARE_ARG, 0x70),
        (OP_PREPARE_EXT_DATA, MACRO_ID_BASE, PREPARE_ARG, 0x60),
    ] {
        let bytes = Command::new(opcode, b1, b2).to_bytes();
        assert_eq!(
            bytes[7], want,
            "{opcode:#04x} {b1:#04x} {b2:#04x} checksummed to {:#04x}, want {want:#04x}",
            bytes[7]
        );
    }
}

#[test]
fn a_command_that_wraps_the_byte_sum_still_checksums() {
    // `0xFF - (sum & 0xFF)`, not `0xFF - sum` — a command whose first seven
    // bytes exceed 255 must still produce a byte, and the total must still
    // land on 0xff mod 256.
    let bytes = Command::new(0xff, 0xff, 0xff).to_bytes();
    let sum: u32 = bytes.iter().map(|&b| u32::from(b)).sum();

    assert_eq!(sum % 0x100, 0xff, "{bytes:02x?}");
}

// -------------------------------------------------------------------- dpi ---

#[test]
fn every_calibration_point_maps_to_its_own_byte() {
    // Interpolation must be exact at the table's own entries.
    for (dpi, byte) in DPI_CALIBRATION {
        assert_eq!(
            dpi_to_byte(dpi),
            byte,
            "{dpi} dpi should be {byte:#04x}"
        );
    }
}

#[test]
fn dpi_between_calibration_points_is_interpolated() {
    // Computed with upstream's algorithm over upstream's table: integer
    // truncation, `b0 + (dpi - d0) * (b1 - b0) / (d1 - d0)`.
    for (dpi, want) in [
        (500u32, 0x0a),
        (1000, 0x16),
        (1500, 0x21),
        (2500, 0x39),
        (5000, 0x7a),
        (7000, 0xd0),
        (9000, 0xe7),
    ] {
        assert_eq!(dpi_to_byte(dpi), want, "{dpi} dpi");
    }
}

#[test]
fn dpi_outside_the_table_is_clamped_not_wrapped() {
    for dpi in [0u32, 1, 100, 199] {
        assert_eq!(dpi_to_byte(dpi), 0x04, "{dpi} dpi should clamp to DPI_MIN");
    }
    for dpi in [10_001u32, 12_000, 65_535, u32::MAX] {
        assert_eq!(dpi_to_byte(dpi), 0xf3, "{dpi} dpi should clamp to DPI_MAX");
    }
}

#[test]
fn dpi_is_monotonic_across_the_whole_range() {
    // A dip anywhere would mean asking for more dpi quietly gave less.
    let mut previous = dpi_to_byte(0);

    for dpi in (0..=11_000).step_by(25) {
        let byte = dpi_to_byte(dpi);
        assert!(
            byte >= previous,
            "{dpi} dpi gave {byte:#04x} after {previous:#04x}"
        );
        previous = byte;
    }
}

#[test]
fn table_bytes_read_back_as_a_plausible_dpi() {
    // `dpi_from_byte` only has to be presentable — there is no readback on
    // this device, so it exists to describe the defaults we wrote. Exactness
    // is not required and is not asserted.
    for (dpi, byte) in DPI_CALIBRATION {
        assert_eq!(dpi_from_byte(byte), dpi, "{byte:#04x} is a table point");
    }

    for byte in [0x00u8, 0x05, 0x50, 0xff] {
        let dpi = dpi_from_byte(byte);
        assert!(
            (DPI_MIN..=DPI_MAX).contains(&dpi),
            "{byte:#04x} read back as {dpi} dpi, outside {DPI_MIN}..={DPI_MAX}"
        );
    }
}

// --------------------------------------------------------- polling / index ---

#[test]
fn the_polling_byte_is_the_interval_in_milliseconds() {
    // 1000 / hz. The byte looks like a bitmask (1, 2, 4, 8) but is not one.
    for (hz, want) in [(1000u32, 1u8), (500, 2), (250, 4), (125, 8)] {
        assert_eq!(polling_interval(hz).unwrap(), want, "{hz} Hz");
    }
}

#[test]
fn every_advertised_report_rate_has_an_interval() {
    for hz in REPORT_RATES {
        let interval = polling_interval(hz)
            .unwrap_or_else(|err| panic!("{hz} Hz is advertised but rejected: {err}"));
        assert_eq!(
            1000 / u32::from(interval),
            hz,
            "{hz} Hz round-trips through interval {interval}"
        );
    }
}

#[test]
fn an_unsupported_report_rate_is_rejected() {
    // The device ignores a bad interval silently, so a rate it cannot do has
    // to fail here rather than look like it worked.
    for hz in [0u32, 1, 100, 200, 333, 750, 2000, 8000, u32::MAX] {
        assert!(
            polling_interval(hz).is_err(),
            "{hz} Hz is not in REPORT_RATES and must be rejected"
        );
    }
}

#[test]
fn profile_indices_are_one_based_on_the_wire() {
    assert_eq!(wire_profile(0).unwrap(), 1);
    assert_eq!(wire_profile(1).unwrap(), 2);
}

#[test]
fn a_profile_index_the_device_does_not_have_is_rejected() {
    // There is no bounds checking in the firmware; writing past profile 2
    // corrupts something upstream could not identify.
    for index in [NUM_PROFILES as u32, 3, 255, u32::MAX] {
        assert!(
            wire_profile(index).is_err(),
            "profile {index} is out of range and must be rejected"
        );
    }
}

// ----------------------------------------------------------------- driver ---

#[test]
fn the_driver_answers_to_the_name_in_the_device_file() {
    // `devices/zelotes-c18.device` says `Driver=c18`, matching upstream's
    // `data/devices/soai-c18.device`. A mismatch here is an
    // `Error::UnknownDriver` the moment the mouse is plugged in.
    assert_eq!(C18Driver.id(), "c18");
}

#[test]
fn the_backend_starts_with_a_table_per_profile() {
    let backend = C18Backend {
        dpi_table: [[0; PAYLOAD_LEN]; NUM_PROFILES],
        button_table: [[0; PAYLOAD_LEN]; NUM_PROFILES],
        next_macro_id: [MACRO_ID_BASE; NUM_PROFILES],
    };

    assert_eq!(backend.dpi_table.len(), NUM_PROFILES);
    assert_eq!(backend.button_table.len(), NUM_PROFILES);
    assert_eq!(backend.next_macro_id, [MACRO_ID_BASE; NUM_PROFILES]);
}
