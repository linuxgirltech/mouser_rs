//! HID++ 2.0 message encoding.
//!
//! All of `impl Message` is pure byte manipulation, so every test here runs
//! without hardware. Write these before `Hidpp20::request`: once a real device
//! is in the picture, a wrong constant and a hardware fault look identical.
//!
//! ## Where the vectors come from
//!
//! [`PING_REQUEST`] and [`PING_REPLY`] are not invented. They were captured
//! from a wired Logitech G502 HERO on `/dev/hidraw1` by
//! `tests/hardware.rs::the_mouse_answers_a_hidpp_protocol_ping`, which writes
//! the request and prints what comes back. The device answered, so these are
//! an external oracle: if `to_bytes` disagrees with `PING_REQUEST`, the
//! encoder is wrong, not the test.
//!
//! The lengths are independently confirmed by the device's own HID report
//! descriptor, which declares report `0x10` with 6 payload bytes and report
//! `0x11` with 19 — 7 and 20 bytes once the report ID is counted.
//!
//! ## What these tests deliberately do not assert
//!
//! Which [`libmouser::Error`] variant a rejection produces, or what its
//! message says. The bodies are unwritten and that is your choice to make;
//! these tests check only that a bad input *is* rejected. Tighten them once
//! the variants are settled.

use libmouser::driver::hidpp20::{
    DEVICE_INDEX_WIRED, FEATURE_ROOT, LONG_LEN, Message, REPORT_ID_LONG, REPORT_ID_SHORT,
    SHORT_LEN,
};

/// Bytes before the parameters: report id, device index, feature index,
/// function-and-software-id. A literal, so a change to `HEADER_LEN` in the
/// source has to be a deliberate decision rather than a silent one.
const HEADER_LEN: usize = 4;

/// Root feature, `getProtocolVersion` (function 1), software id `0x0a`,
/// arbitrary ping byte `0x5a`. Captured going out to a real G502.
const PING_REQUEST: [u8; SHORT_LEN] = [0x10, 0xff, 0x00, 0x1a, 0x00, 0x00, 0x5a];

/// What the G502 sent back: HID++ **4.2**, ping byte echoed. Note the device
/// answered a *short* request with a *long* report — nothing in `Message` may
/// assume the two match.
const PING_REPLY: [u8; LONG_LEN] = [
    0x11, 0xff, 0x00, 0x1a, 0x04, 0x02, 0x5a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00,
];

/// The ping request, built rather than quoted.
fn ping() -> Message {
    Message::short(DEVICE_INDEX_WIRED, 0x00, 1, &[0x00, 0x00, 0x5a])
        .expect("the ping is a valid short message")
}

// ------------------------------------------------------------- the wire ---

#[test]
fn the_report_lengths_are_what_the_device_declares() {
    // 6 payload bytes + 1 report id, and 19 + 1, from the G502's own report
    // descriptor. Literals on purpose: these cross the wire.
    assert_eq!(SHORT_LEN, 7);
    assert_eq!(LONG_LEN, 20);
    assert_eq!(REPORT_ID_SHORT, 0x10);
    assert_eq!(REPORT_ID_LONG, 0x11);
    assert_eq!(DEVICE_INDEX_WIRED, 0xff);
    assert_eq!(FEATURE_ROOT, 0x0000);
    assert_eq!(Message::HEADER_LEN, HEADER_LEN);
}

#[test]
fn the_software_id_is_non_zero() {
    // Zero marks unsolicited device notifications. A zero software id would
    // make our own requests indistinguishable from them, and every reply
    // would be discarded as a notification.
    assert_ne!(Message::SOFTWARE_ID, 0);
    assert!(
        Message::SOFTWARE_ID <= 0x0f,
        "the software id has to fit the low nibble, got {:#04x}",
        Message::SOFTWARE_ID
    );
}

// ------------------------------------------------------------ param_len ---

#[test]
fn param_capacity_is_the_report_length_less_the_header() {
    assert_eq!(Message::param_len(REPORT_ID_SHORT).unwrap(), 3);
    assert_eq!(Message::param_len(REPORT_ID_LONG).unwrap(), 16);

    // Stated again in terms of the constants, so a change to either length
    // cannot leave the header size silently wrong.
    assert_eq!(
        Message::param_len(REPORT_ID_SHORT).unwrap(),
        SHORT_LEN - HEADER_LEN
    );
    assert_eq!(
        Message::param_len(REPORT_ID_LONG).unwrap(),
        LONG_LEN - HEADER_LEN
    );
}

#[test]
fn an_unknown_report_id_has_no_param_capacity() {
    // Including 0x12 and 0x0f, which sit either side of the two real ones,
    // and 0xff, which is a valid *device index* and a valid error *feature
    // index* but never a report id.
    for report_id in [0x00, 0x01, 0x0f, 0x12, 0x20, 0x8f, 0xff] {
        assert!(
            Message::param_len(report_id).is_err(),
            "{report_id:#04x} should not be a HID++ report id"
        );
    }
}

// -------------------------------------------------------- construction ---

#[test]
fn a_short_message_carries_the_fields_it_was_given() {
    let msg = Message::short(DEVICE_INDEX_WIRED, 0x07, 3, &[0xaa, 0xbb, 0xcc]).unwrap();

    assert_eq!(msg.report_id, REPORT_ID_SHORT);
    assert_eq!(msg.device_index, DEVICE_INDEX_WIRED);
    assert_eq!(msg.feature_index, 0x07);
    assert_eq!(msg.params, vec![0xaa, 0xbb, 0xcc]);
}

#[test]
fn a_long_message_carries_the_fields_it_was_given() {
    let params: Vec<u8> = (0..16).collect();
    let msg = Message::long(0x01, 0x07, 3, &params).unwrap();

    assert_eq!(msg.report_id, REPORT_ID_LONG);
    assert_eq!(msg.device_index, 0x01, "a device behind a receiver");
    assert_eq!(msg.feature_index, 0x07);
    assert_eq!(msg.params, params);
}

#[test]
fn short_params_are_zero_padded_to_capacity() {
    // The protocol expects the full report either way, so a caller passing
    // fewer params gets zeros rather than an error.
    assert_eq!(
        Message::short(DEVICE_INDEX_WIRED, 0, 0, &[]).unwrap().params,
        vec![0, 0, 0]
    );
    assert_eq!(
        Message::short(DEVICE_INDEX_WIRED, 0, 0, &[0x01])
            .unwrap()
            .params,
        vec![0x01, 0, 0]
    );
}

#[test]
fn long_params_are_zero_padded_to_capacity() {
    let msg = Message::long(DEVICE_INDEX_WIRED, 0, 0, &[0x01, 0x02]).unwrap();

    assert_eq!(msg.params.len(), 16);
    assert_eq!(&msg.params[..2], &[0x01, 0x02]);
    assert!(
        msg.params[2..].iter().all(|&b| b == 0),
        "the tail should be zeros, got {:02x?}",
        msg.params
    );
}

#[test]
fn params_that_do_not_fit_the_report_are_rejected() {
    // Truncating would send a different request than the caller asked for.
    assert!(
        Message::short(DEVICE_INDEX_WIRED, 0, 0, &[1, 2, 3, 4]).is_err(),
        "4 params do not fit a short report"
    );
    assert!(
        Message::long(DEVICE_INDEX_WIRED, 0, 0, &[0u8; 17]).is_err(),
        "17 params do not fit a long report"
    );
}

#[test]
fn params_that_exactly_fill_the_report_are_accepted() {
    assert!(Message::short(DEVICE_INDEX_WIRED, 0, 0, &[1, 2, 3]).is_ok());
    assert!(Message::long(DEVICE_INDEX_WIRED, 0, 0, &[0u8; 16]).is_ok());
}

// ------------------------------------------------- the function nibble ---

#[test]
fn the_function_byte_packs_the_function_above_the_software_id() {
    let msg = Message::short(DEVICE_INDEX_WIRED, 0, 1, &[]).unwrap();
    assert_eq!(msg.function, 0x1a, "function 1, software id 0x0a");

    assert_eq!(
        msg.function >> 4,
        1,
        "the function belongs in the high nibble"
    );
    assert_eq!(
        msg.function & 0x0f,
        Message::SOFTWARE_ID,
        "the software id belongs in the low nibble"
    );
}

#[test]
fn every_valid_function_nibble_survives_the_packing() {
    // Catches an off-by-one shift, which would alias several functions onto
    // one byte and be near-impossible to see on the wire.
    for function in 0u8..=0x0f {
        let msg = Message::short(DEVICE_INDEX_WIRED, 0, function, &[]).unwrap();

        assert_eq!(
            msg.function >> 4,
            function,
            "function {function:#x} did not round-trip through {:#04x}",
            msg.function
        );
        assert_eq!(msg.function & 0x0f, Message::SOFTWARE_ID);
    }
}

#[test]
fn a_function_wider_than_a_nibble_is_rejected() {
    // `(0x10 << 4) & 0xff == 0`, which on the root feature is `getFeature` —
    // a real call returning plausible garbage. Silently wrong is the worst
    // outcome here, so it has to be an error.
    for function in [0x10, 0x11, 0x1f, 0xa0, 0xff] {
        assert!(
            Message::short(DEVICE_INDEX_WIRED, 0, function, &[]).is_err(),
            "function {function:#04x} does not fit a nibble and must be rejected"
        );
        assert!(
            Message::long(DEVICE_INDEX_WIRED, 0, function, &[]).is_err(),
            "function {function:#04x} does not fit a nibble and must be rejected"
        );
    }
}

// --------------------------------------------------------- to_bytes ---

#[test]
fn the_ping_encodes_to_the_bytes_the_g502_answered() {
    // The load-bearing test in this file. A round-trip test passes with the
    // header order reversed in both directions; this one does not.
    assert_eq!(ping().to_bytes(), PING_REQUEST);
}

#[test]
fn the_header_is_report_id_device_index_feature_index_function() {
    let bytes = Message::short(0xff, 0x07, 3, &[0xaa, 0xbb, 0xcc])
        .unwrap()
        .to_bytes();

    assert_eq!(bytes[0], REPORT_ID_SHORT, "byte 0: report id");
    assert_eq!(bytes[1], 0xff, "byte 1: device index");
    assert_eq!(bytes[2], 0x07, "byte 2: feature index");
    assert_eq!(bytes[3], 0x3a, "byte 3: function 3 | software id 0x0a");
    assert_eq!(&bytes[4..], &[0xaa, 0xbb, 0xcc], "then the params");
}

#[test]
fn encoding_always_produces_the_full_report_length() {
    // Short of the full length the device ignores the report entirely, and
    // `write(2)` gives no hint that anything was wrong.
    assert_eq!(
        Message::short(DEVICE_INDEX_WIRED, 0, 0, &[])
            .unwrap()
            .to_bytes()
            .len(),
        SHORT_LEN
    );
    assert_eq!(
        Message::long(DEVICE_INDEX_WIRED, 0, 0, &[])
            .unwrap()
            .to_bytes()
            .len(),
        LONG_LEN
    );
}

#[test]
fn an_empty_long_message_is_a_header_and_sixteen_zeros() {
    let bytes = Message::long(DEVICE_INDEX_WIRED, 0x00, 0, &[]).unwrap().to_bytes();

    assert_eq!(bytes.len(), LONG_LEN);
    assert_eq!(&bytes[..4], &[0x11, 0xff, 0x00, 0x0a]);
    assert!(bytes[4..].iter().all(|&b| b == 0));
}

// -------------------------------------------------------- from_bytes ---

#[test]
fn the_real_reply_decodes_to_hidpp_four_two() {
    let reply = Message::from_bytes(&PING_REPLY).expect("the G502's own reply must parse");

    assert_eq!(reply.report_id, REPORT_ID_LONG);
    assert_eq!(reply.device_index, DEVICE_INDEX_WIRED);
    assert_eq!(reply.feature_index, 0x00, "from the root feature");
    assert_eq!(reply.function, 0x1a, "our function and software id, echoed");
    assert_eq!(reply.params.len(), 16);

    assert_eq!(reply.params[0], 4, "HID++ major version");
    assert_eq!(reply.params[1], 2, "HID++ minor version");
    assert_eq!(reply.params[2], 0x5a, "our ping byte, echoed");

    assert!(!reply.is_error());
}

#[test]
fn a_short_reply_decodes_three_params() {
    let bytes = [0x10, 0xff, 0x00, 0x1a, 0x04, 0x02, 0x5a];
    let msg = Message::from_bytes(&bytes).unwrap();

    assert_eq!(msg.report_id, REPORT_ID_SHORT);
    assert_eq!(msg.params, vec![0x04, 0x02, 0x5a]);
}

#[test]
fn decoding_rejects_an_unknown_report_id() {
    // Not every report arriving on the node is HID++ — the same hidraw node
    // carries the G502's keyboard and consumer collections.
    let mut bytes = PING_REPLY;
    bytes[0] = 0x03;
    assert!(Message::from_bytes(&bytes).is_err());
}

#[test]
fn decoding_rejects_a_truncated_report() {
    // A short read is a bug worth seeing, not something to zero-pad over.
    for len in 0..SHORT_LEN {
        assert!(
            Message::from_bytes(&PING_REQUEST[..len]).is_err(),
            "{len} bytes is not a whole short report"
        );
    }

    for len in 0..LONG_LEN {
        assert!(
            Message::from_bytes(&PING_REPLY[..len]).is_err(),
            "{len} bytes is not a whole long report"
        );
    }
}

#[test]
fn decoding_ignores_bytes_past_the_report_length() {
    // Lets a caller hand over a fixed 20-byte buffer without slicing by the
    // count `read_input_report` returned. `request` should still slice; this
    // is a safety net, not a licence.
    let mut padded = [0u8; LONG_LEN];
    padded[..SHORT_LEN].copy_from_slice(&PING_REQUEST);

    let msg = Message::from_bytes(&padded).expect("a short report inside a long buffer");
    assert_eq!(msg.report_id, REPORT_ID_SHORT);
    assert_eq!(msg.params, vec![0x00, 0x00, 0x5a]);
}

// ---------------------------------------------------------- is_error ---

#[test]
fn an_error_marker_in_the_feature_index_is_an_error() {
    for (marker, version) in [(0x8f, "HID++ 1.0"), (0xff, "HID++ 2.0")] {
        let mut bytes = PING_REPLY;
        bytes[2] = marker;

        let msg = Message::from_bytes(&bytes).unwrap();
        assert!(
            msg.is_error(),
            "feature index {marker:#04x} is the {version} error marker"
        );
    }
}

#[test]
fn a_wired_device_index_is_not_an_error() {
    // The trap: 0xff in byte 1 means "directly wired" and is on *every*
    // reply from the G502. 0xff in byte 2 means the request failed. Read the
    // wrong byte and every success looks like a failure.
    let reply = Message::from_bytes(&PING_REPLY).unwrap();

    assert_eq!(reply.device_index, 0xff);
    assert_eq!(reply.feature_index, 0x00);
    assert!(
        !reply.is_error(),
        "a wired reply from the root feature is not an error"
    );
}

#[test]
fn ordinary_feature_indices_are_not_errors() {
    for feature_index in [0x00, 0x01, 0x02, 0x08, 0x0f, 0x7f, 0x8e, 0x90, 0xfe] {
        let mut bytes = PING_REPLY;
        bytes[2] = feature_index;

        assert!(
            !Message::from_bytes(&bytes).unwrap().is_error(),
            "feature index {feature_index:#04x} is not an error marker"
        );
    }
}

#[test]
fn a_request_we_built_is_never_an_error() {
    assert!(!ping().is_error());
}

// -------------------------------------------------------- round trip ---

#[test]
fn encoding_and_decoding_are_inverses() {
    // Weak on its own — it would pass with the header order reversed in both
    // directions. It earns its place only alongside the literal-vector tests
    // above, where it catches a padding or length slip they would not.
    let cases = [
        ping(),
        Message::short(DEVICE_INDEX_WIRED, 0x00, 0x0f, &[0xff, 0xff, 0xff]).unwrap(),
        Message::short(0x01, 0xfe, 0x00, &[]).unwrap(),
        Message::long(DEVICE_INDEX_WIRED, 0x07, 0x05, &(0..16).collect::<Vec<u8>>()).unwrap(),
        Message::long(0x06, 0x00, 0x00, &[]).unwrap(),
    ];

    for msg in cases {
        let bytes = msg.to_bytes();
        let back = Message::from_bytes(&bytes)
            .unwrap_or_else(|err| panic!("{bytes:02x?} failed to decode: {err}"));

        assert_eq!(back, msg, "via {bytes:02x?}");
    }
}
