//! Unit tests for `driver::hidpp20`.
//!
//! Compiled as a child module of `crate::driver::hidpp20` via `#[path]`, so
//! the private `Hidpp20::is_reply_to` is in scope without being `pub`. Cargo
//! only auto-discovers `tests/*.rs` and `tests/*/main.rs`, so this file is
//! never built as a standalone integration test.
//!
//! `impl Message`'s public surface is covered separately in
//! `tests/hidpp20.rs`. What is here is the reply matcher, which is the part
//! most likely to be wrong and the part hardware cannot exercise: you cannot
//! make a mouse emit an unsolicited notification on demand.
//!
//! ## Where the vectors come from
//!
//! Four complete exchanges captured from a wired Logitech G502 HERO on its
//! HID++ node. Each `*_REQUEST` is bytes that went out; each `*_REPLY` is
//! what came back. They are an external oracle — the device answered.

use super::*;

// ---------------------------------------------------------------- vectors ---

/// Root feature, `getProtocolVersion`, ping byte `0x5a`.
const VERSION_REQUEST: [u8; 7] = [0x10, 0xff, 0x00, 0x1a, 0x00, 0x00, 0x5a];
/// HID++ 4.2, ping echoed. Note the long reply to a short request.
const VERSION_REPLY: [u8; 20] = [
    0x11, 0xff, 0x00, 0x1a, 0x04, 0x02, 0x5a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Root feature, `getFeature(0x2201)` — AdjustableDpi. Feature id big-endian.
const GET_FEATURE_REQUEST: [u8; 7] = [0x10, 0xff, 0x00, 0x0a, 0x22, 0x01, 0x00];
/// Index 10, type 0, version 1.
const GET_FEATURE_REPLY: [u8; 20] = [
    0x11, 0xff, 0x00, 0x0a, 0x0a, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Root feature, function `0x0f` — which does not exist.
const BAD_FUNCTION_REQUEST: [u8; 7] = [0x10, 0xff, 0x00, 0xfa, 0x00, 0x00, 0x00];
/// Error: marker `0xff`, failed feature `0x00`, failed function `0xfa`,
/// code 7 (invalid function id).
const BAD_FUNCTION_REPLY: [u8; 20] = [
    0x11, 0xff, 0xff, 0x00, 0xfa, 0x07, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Feature index `0x7f`, which this device does not have.
const BAD_FEATURE_REQUEST: [u8; 7] = [0x10, 0xff, 0x7f, 0x0a, 0x00, 0x00, 0x00];
/// Error: failed feature `0x7f`, failed function `0x0a`, code 6 (invalid
/// feature index).
const BAD_FEATURE_REPLY: [u8; 20] = [
    0x11, 0xff, 0xff, 0x7f, 0x0a, 0x06, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn parse(bytes: &[u8]) -> Message {
    Message::from_bytes(bytes).expect("a captured report must parse")
}

/// Every captured exchange, as (name, request, reply).
fn exchanges() -> Vec<(&'static str, Message, Message)> {
    vec![
        (
            "getProtocolVersion",
            parse(&VERSION_REQUEST),
            parse(&VERSION_REPLY),
        ),
        (
            "getFeature",
            parse(&GET_FEATURE_REQUEST),
            parse(&GET_FEATURE_REPLY),
        ),
        (
            "invalid function",
            parse(&BAD_FUNCTION_REQUEST),
            parse(&BAD_FUNCTION_REPLY),
        ),
        (
            "invalid feature index",
            parse(&BAD_FEATURE_REQUEST),
            parse(&BAD_FEATURE_REPLY),
        ),
    ]
}

/// An unsolicited notification: software id 0 in the low nibble.
fn notification(feature_index: u8, function_nibble: u8) -> Message {
    let mut bytes = [0u8; 20];
    bytes[0] = REPORT_ID_LONG;
    bytes[1] = DEVICE_INDEX_WIRED;
    bytes[2] = feature_index;
    bytes[3] = function_nibble << 4; // software id 0
    parse(&bytes)
}

// ------------------------------------------------------- real exchanges ---

#[test]
fn every_captured_reply_matches_its_own_request() {
    for (name, request, reply) in exchanges() {
        assert!(
            Hidpp20::is_reply_to(&reply, &request),
            "{name}: the device's own reply was not recognised\n  request {:02x?}\n  reply   {:02x?}",
            request.to_bytes(),
            reply.to_bytes()
        );
    }
}

#[test]
fn no_captured_reply_matches_a_different_request() {
    // Each reply against every *other* request. Catches a matcher that is
    // merely permissive rather than correct — the version and getFeature
    // exchanges share a feature index, and the two errors share a device
    // index and an error marker.
    let all = exchanges();

    for (reply_name, _, reply) in &all {
        for (request_name, request, _) in &all {
            if reply_name == request_name {
                continue;
            }
            assert!(
                !Hidpp20::is_reply_to(reply, request),
                "the {reply_name} reply must not answer the {request_name} request"
            );
        }
    }
}

// --------------------------------------------------- the two match arms ---

#[test]
fn a_normal_reply_matches_on_feature_index_and_function() {
    let request = parse(&VERSION_REQUEST);
    let reply = parse(&VERSION_REPLY);

    assert!(!reply.is_error());
    assert_eq!(reply.feature_index, request.feature_index);
    assert_eq!(reply.function, request.function);
    assert!(Hidpp20::is_reply_to(&reply, &request));
}

#[test]
fn an_error_reply_matches_despite_a_different_feature_index() {
    // This is why the matcher needs two arms. The error marker *occupies* the
    // feature-index position, so the normal criteria can never hold: the
    // reply's feature index is 0xff and the request's is 0x00. A single-arm
    // matcher discards this as a notification and then reports a timeout for
    // a device that answered immediately.
    let request = parse(&BAD_FUNCTION_REQUEST);
    let reply = parse(&BAD_FUNCTION_REPLY);

    assert!(reply.is_error());
    assert_ne!(
        reply.feature_index, request.feature_index,
        "0xff marker vs feature 0x00"
    );
    assert_ne!(
        reply.function, request.function,
        "the error echoes the feature index here, not the function"
    );

    assert!(Hidpp20::is_reply_to(&reply, &request));
}

#[test]
fn an_error_reply_echoes_the_request_in_the_shifted_positions() {
    // Documents the layout the error arm depends on, so a change to it fails
    // here with an explanation rather than somewhere in `request`.
    for (request_bytes, reply_bytes, code) in [
        (&BAD_FUNCTION_REQUEST, &BAD_FUNCTION_REPLY, 7u8),
        (&BAD_FEATURE_REQUEST, &BAD_FEATURE_REPLY, 6),
    ] {
        let request = parse(request_bytes);
        let reply = parse(reply_bytes);

        assert_eq!(
            reply.function, request.feature_index,
            "byte 3 of an error is the failed request's feature index"
        );
        assert_eq!(
            reply.params[0], request.function,
            "params[0] is the failed request's function|software id"
        );
        assert_eq!(reply.params[1], code, "params[1] is the error code");
    }
}

#[test]
fn the_software_id_of_an_error_is_not_where_it_normally_lives() {
    // The reason the error arm reads `params[0]` instead of `function`.
    let reply = parse(&BAD_FUNCTION_REPLY);

    assert_ne!(
        reply.function & 0x0f,
        Message::SOFTWARE_ID,
        "the low nibble of `function` is not our software id on an error"
    );
    assert_eq!(
        reply.params[0] & 0x0f,
        Message::SOFTWARE_ID,
        "it is down in params[0]"
    );
}

// --------------------------------------------------------- rejections ---

#[test]
fn a_reply_for_another_device_is_rejected() {
    // Devices 0x01..0x06 sit behind a unifying receiver; 0xff is wired. A
    // receiver multiplexes all of them onto one node, so this is the first
    // thing to check.
    let request = parse(&VERSION_REQUEST);
    assert_eq!(request.device_index, DEVICE_INDEX_WIRED);

    for device_index in [0x00, 0x01, 0x02, 0x06, 0xfe] {
        let mut bytes = VERSION_REPLY;
        bytes[1] = device_index;

        assert!(
            !Hidpp20::is_reply_to(&parse(&bytes), &request),
            "a reply from device {device_index:#04x} must not answer device 0xff"
        );
    }
}

#[test]
fn a_notification_is_never_a_reply() {
    // Software id 0 marks unsolicited device traffic. The matcher has no
    // explicit zero check — it compares the whole `function` byte, and our
    // requests always carry a non-zero software id, so a notification can
    // never collide. This pins that reasoning.
    let request = parse(&VERSION_REQUEST);

    for function_nibble in 0u8..=0x0f {
        let note = notification(request.feature_index, function_nibble);

        assert_eq!(note.function & 0x0f, 0, "software id 0");
        assert!(
            !Hidpp20::is_reply_to(&note, &request),
            "a notification with function nibble {function_nibble:#x} was taken for a reply"
        );
    }
}

#[test]
fn a_reply_about_a_different_feature_is_rejected() {
    // Two features can be mid-conversation with the same function number.
    let request = parse(&GET_FEATURE_REQUEST);

    for feature_index in [0x01, 0x02, 0x0a, 0x0c, 0x7f] {
        let mut bytes = GET_FEATURE_REPLY;
        bytes[2] = feature_index;

        assert!(
            !Hidpp20::is_reply_to(&parse(&bytes), &request),
            "a reply from feature {feature_index:#04x} must not answer feature 0x00"
        );
    }
}

#[test]
fn a_reply_to_a_different_function_is_rejected() {
    // Same feature, different function: getFeature (0) vs
    // getProtocolVersion (1), both on the root feature.
    let request = parse(&GET_FEATURE_REQUEST);
    let reply = parse(&VERSION_REPLY);

    assert_eq!(reply.feature_index, request.feature_index, "both root");
    assert_ne!(reply.function, request.function);
    assert!(!Hidpp20::is_reply_to(&reply, &request));
}

#[test]
fn an_error_for_a_different_request_is_rejected() {
    // Both errors come from device 0xff with marker 0xff. Only the echoed
    // fields tell them apart, so a matcher that stops at `is_error()` would
    // accept either for either.
    let bad_function = (parse(&BAD_FUNCTION_REQUEST), parse(&BAD_FUNCTION_REPLY));
    let bad_feature = (parse(&BAD_FEATURE_REQUEST), parse(&BAD_FEATURE_REPLY));

    assert!(!Hidpp20::is_reply_to(&bad_feature.1, &bad_function.0));
    assert!(!Hidpp20::is_reply_to(&bad_function.1, &bad_feature.0));
}

#[test]
fn an_error_with_no_params_is_rejected_without_panicking() {
    // `params.first()` on a message whose params were emptied by hand. A
    // matcher indexing `params[0]` directly would panic here instead.
    let mut reply = parse(&BAD_FUNCTION_REPLY);
    reply.params.clear();

    assert!(!Hidpp20::is_reply_to(&reply, &parse(&BAD_FUNCTION_REQUEST)));
}

// ----------------------------------------------------------- properties ---

#[test]
fn matching_does_not_depend_on_the_reply_report_length() {
    // The G502 answers short requests with long reports. Nothing in the
    // matcher may key off the report id, so the same exchange must match
    // whichever length it arrives in.
    let request = parse(&VERSION_REQUEST);

    let long = parse(&VERSION_REPLY);

    // The same header and params as a short report. Byte 0 has to change
    // too: truncating a 0x11 report to 7 bytes is a malformed long report,
    // not a short one, and `from_bytes` rightly rejects it.
    let mut short_bytes = [0u8; 7];
    short_bytes.copy_from_slice(&VERSION_REPLY[..7]);
    short_bytes[0] = REPORT_ID_SHORT;
    let short = parse(&short_bytes);

    assert_eq!(long.report_id, REPORT_ID_LONG);
    assert_eq!(short.report_id, REPORT_ID_SHORT);

    assert!(Hidpp20::is_reply_to(&long, &request));
    assert!(Hidpp20::is_reply_to(&short, &request));
}

#[test]
fn a_request_is_not_a_reply_to_itself() {
    // A loopback or an echoing device must not satisfy the matcher. Our
    // requests are short reports with the feature index in byte 2; a reply
    // has it there too, so this one genuinely *does* match on the normal arm
    // — which is why `request` must not read back its own write. Pinned so
    // the hazard is recorded rather than discovered.
    let request = parse(&VERSION_REQUEST);

    assert!(
        Hidpp20::is_reply_to(&request, &request),
        "a request looks like its own reply: `request` must never read back \
         what it wrote, and hidraw does not echo, so this is latent only"
    );
}
