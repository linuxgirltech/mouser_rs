//! Tests that need real hardware. All `#[ignore]`d — run them with:
//!
//! ```text
//! cargo test --test hardware -- --ignored --nocapture
//! ```
//!
//! They need read-write access to the hidraw nodes, which
//! `data/udev/70-mouser-dev.rules` grants for the G502.

#![cfg(feature = "udev")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use libmouser::driver::c18::{C18, Command, NUM_RESOLUTIONS, OP_SET_DPI_STAGE};
use libmouser::driver::hidpp20::{
    DEVICE_INDEX_WIRED, FEATURE_ADJUSTABLE_DPI, FEATURE_COLOR_LED_EFFECTS,
    FEATURE_DEVICE_FW_VERSION, FEATURE_FEATURE_SET, FEATURE_ONBOARD_PROFILES, FEATURE_REPORT_RATE,
    FEATURE_ROOT, Hidpp20, Message,
};
use libmouser::enumerate::{HotplugEvent, Monitor, enumerate};
use libmouser::hid::HID_MAX_DESCRIPTOR_SIZE;
use libmouser::{BusType, HidrawDevice};

/// Vendor and product of the development device.
const G502: (u16, u16) = (0x046d, 0xc08b);

/// Read a node's HID report descriptor out of sysfs.
///
/// `HidrawNode::syspath` points at `/sys/.../hidraw/hidrawN`, whose `device`
/// symlink is the HID device that owns the descriptor.
fn report_descriptor(syspath: &Path) -> Option<Vec<u8>> {
    std::fs::read(syspath.join("device/report_descriptor")).ok()
}

/// True when the descriptor declares a vendor-defined collection carrying
/// HID++ report `0x10`.
///
/// `06 00 ff` is Usage Page (Vendor-Defined 0xFF00) and `85 10` is
/// Report ID (0x10). A device's boot-mouse interface has neither, which is
/// what separates the two nodes one physical mouse presents.
fn speaks_hidpp(descriptor: &[u8]) -> bool {
    let has = |needle: &[u8]| descriptor.windows(needle.len()).any(|w| w == needle);
    has(&[0x06, 0x00, 0xff]) && has(&[0x85, 0x10])
}

/// Every node belonging to `vendor:product`.
fn nodes_for(vendor: u16, product: u16) -> Vec<PathBuf> {
    enumerate()
        .expect("enumerate should work on a live system")
        .into_iter()
        .filter(|node| match HidrawDevice::open(&node.devnode) {
            Ok(dev) => {
                let info = dev.info();
                info.vendor == vendor && info.product == product
            }
            Err(_) => false,
        })
        .map(|node| node.devnode)
        .collect()
}

/// The one node for `vendor:product` that speaks HID++.
fn hidpp_node(vendor: u16, product: u16) -> Option<PathBuf> {
    enumerate()
        .expect("enumerate should work on a live system")
        .into_iter()
        .find(|node| {
            let Ok(dev) = HidrawDevice::open(&node.devnode) else {
                return false;
            };
            let info = dev.info();
            if (info.vendor, info.product) != (vendor, product) {
                return false;
            }
            report_descriptor(&node.syspath).is_some_and(|d| speaks_hidpp(&d))
        })
        .map(|node| node.devnode)
}

#[test]
#[ignore = "requires real hardware"]
fn enumerate_finds_nodes_and_they_are_sorted() {
    let nodes = enumerate().expect("enumerate failed");
    assert!(!nodes.is_empty(), "no hidraw nodes on this system");

    let paths: Vec<_> = nodes.iter().map(|n| &n.devnode).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "enumerate must return a deterministic order");

    for node in &nodes {
        assert!(node.devnode.starts_with("/dev/"), "{:?}", node.devnode);
        assert!(node.syspath.starts_with("/sys/"), "{:?}", node.syspath);
    }
}

#[test]
#[ignore = "requires real hardware"]
fn every_node_reports_a_plausible_identity() {
    for node in enumerate().unwrap() {
        let Ok(dev) = HidrawDevice::open(&node.devnode) else {
            continue; // no permission for this one; not what we are testing
        };

        let info = dev.info();
        assert_eq!(dev.path(), node.devnode);
        assert!(
            matches!(info.bus, BusType::Usb | BusType::Bluetooth | BusType::Unknown),
            "{:?}",
            info.bus
        );
        assert!(
            !info.name.is_empty(),
            "{} reported an empty HID name",
            node.devnode.display()
        );
        assert!(
            !info.name.contains('\0'),
            "{} name was not truncated at the NUL",
            node.devnode.display()
        );
        assert!(
            !info.phys.contains('\0'),
            "{} phys was not truncated at the NUL",
            node.devnode.display()
        );
    }
}

#[test]
#[ignore = "requires the G502"]
fn one_physical_mouse_presents_several_hidraw_nodes() {
    // The duplicate-device problem, asserted rather than assumed: the database
    // matches on bus/vid/pid, which cannot tell these apart. Only probing can.
    let nodes = nodes_for(G502.0, G502.1);
    assert!(
        nodes.len() > 1,
        "expected more than one node for the G502, found {nodes:?}"
    );
    println!("G502 nodes: {nodes:?}");
}

#[test]
#[ignore = "requires the G502"]
fn exactly_one_g502_node_declares_the_hidpp_collection() {
    let all = nodes_for(G502.0, G502.1);
    assert!(!all.is_empty(), "the G502 was not found");

    let hidpp: Vec<_> = enumerate()
        .unwrap()
        .into_iter()
        .filter(|node| {
            HidrawDevice::open(&node.devnode)
                .is_ok_and(|d| (d.info().vendor, d.info().product) == G502)
                && report_descriptor(&node.syspath).is_some_and(|d| speaks_hidpp(&d))
        })
        .map(|node| node.devnode)
        .collect();

    assert_eq!(
        hidpp.len(),
        1,
        "expected exactly one HID++ node out of {all:?}, got {hidpp:?}"
    );
    println!("HID++ node: {}", hidpp[0].display());
}

#[test]
#[ignore = "requires the G502"]
fn the_mouse_answers_a_hidpp_protocol_ping() {
    let path = hidpp_node(G502.0, G502.1).expect("no HID++ node found for the G502");
    println!("pinging {}", path.display());

    // Root feature (index 0x00), function 1 = getProtocolVersion, software id
    // 0x0a in the low nibble. The final byte is echoed back, which is what
    // distinguishes a reply from an unsolicited notification.
    const PING: u8 = 0x5a;
    let request = [0x10u8, 0xff, 0x00, 0x1a, 0x00, 0x00, PING];

    // `read_input_report` is a blocking `read(2)` and `HidrawDevice` exposes no
    // timeout, so the read happens on a thread we can abandon. This is exactly
    // the hazard `Hidpp20::request` has to solve properly.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = HidrawDevice::open(&path).and_then(|mut dev| {
            dev.write_output_report(&request)?;
            let mut buf = [0u8; 20];
            let n = dev.read_input_report(&mut buf)?;
            Ok(buf[..n].to_vec())
        });
        let _ = tx.send(result);
    });

    let reply = match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(reply)) => reply,
        Ok(Err(err)) => panic!("hidraw error: {err}"),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("no reply within 2s — wrong node, or the device is asleep")
        }
        // The sender is dropped when the worker panics, which arrives as
        // `Disconnected` immediately. Reporting it as a timeout would send
        // you looking for a hardware fault instead of reading the panic.
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("the worker thread panicked; its message is above")
        }
    };

    println!("reply: {reply:02x?}");

    // The reply's report ID need not match the request's: a G502 answers a
    // short (0x10) ping with a long (0x11) report. `Hidpp20::request` must
    // therefore read into a 20-byte buffer and take the length from the
    // report ID that came back, never from the one it sent.
    assert!(
        matches!(reply[0], 0x10 | 0x11),
        "unexpected report id: {reply:02x?}"
    );
    let expected_len = if reply[0] == 0x10 { 7 } else { 20 };
    assert_eq!(
        reply.len(),
        expected_len,
        "report {:#04x} should be {expected_len} bytes: {reply:02x?}",
        reply[0]
    );

    assert_eq!(reply[1], 0xff, "expected the wired device index");
    assert_ne!(
        reply[2], 0x8f,
        "device answered with a HID++ 1.0 error: {reply:02x?}"
    );
    assert_ne!(
        reply[2], 0xff,
        "device answered with a HID++ 2.0 error: {reply:02x?}"
    );
    assert_eq!(reply[2], 0x00, "reply should come from the root feature");
    assert_eq!(reply[3], 0x1a, "function and software id should be echoed");
    assert_eq!(reply[6], PING, "ping byte should be echoed back");

    let (major, minor) = (reply[4], reply[5]);
    println!("HID++ version {major}.{minor}");
    assert!(major >= 2, "expected HID++ 2.0 or later, got {major}.{minor}");
}

#[test]
#[ignore = "requires real hardware"]
fn the_report_descriptor_filter_rejects_a_plain_mouse_interface() {
    // Guards the helper the two tests above rely on: at least one node on this
    // system must *fail* the HID++ check, or the filter is matching everything.
    let nodes = enumerate().unwrap();
    let descriptors: Vec<_> = nodes
        .iter()
        .filter_map(|n| report_descriptor(&n.syspath))
        .collect();

    assert!(!descriptors.is_empty(), "no descriptors readable");
    assert!(
        descriptors.iter().any(|d| !speaks_hidpp(d)),
        "every node claimed to speak HID++, which cannot be right"
    );
}

#[test]
#[ignore = "unplug and replug the mouse within 30s"]
fn the_monitor_sees_the_mouse_come_and_go() {
    fn saw(seen: &[(HotplugEvent, PathBuf)], kind: HotplugEvent) -> bool {
        seen.iter().any(|(k, _)| *k == kind)
    }

    let mut monitor = Monitor::new().expect("monitor should open");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut seen: Vec<(HotplugEvent, PathBuf)> = Vec::new();

    eprintln!("unplug the mouse, then plug it back in...");
    while std::time::Instant::now() < deadline {
        while let Some((kind, node)) = monitor.poll_event() {
            eprintln!("{kind:?} {}", node.devnode.display());
            seen.push((kind, node.devnode));
        }

        // Stop once both kinds have arrived, never on a count: one unplug of
        // a device presenting several nodes already yields several Removed
        // events, so `seen.len() >= 2` is satisfied before the replug.
        if saw(&seen, HotplugEvent::Removed) && saw(&seen, HotplugEvent::Added) {
            break;
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(!seen.is_empty(), "no hotplug events in 30s — did you unplug it?");
    assert!(
        saw(&seen, HotplugEvent::Removed),
        "expected a Removed event, got {seen:?}"
    );
    assert!(
        saw(&seen, HotplugEvent::Added),
        "expected an Added event, got {seen:?}"
    );

    // The node numbering is not stable across a replug: the kernel assigns
    // hidraw indices in probe order, so the device can come back on different
    // ones. Deliberately not asserted.
    eprintln!("{} events total", seen.len());
    assert!(
        seen.iter().any(|(k, _)| *k == HotplugEvent::Added),
        "expected an Added event, got {seen:?}"
    );
}
// =========================================================== Hidpp20 ======
//
// Everything below talks to the device. The vectors these assert against were
// captured from a wired G502 HERO; see the comments for the raw bytes.

/// Open the G502's HID++ node, ready for a conversation.
///
/// Finds the node by report descriptor rather than by name: the kernel
/// assigns hidraw indices in probe order, so this mouse has been `hidraw1`
/// and `hidraw4` across reboots of the same machine.
fn g502_hidpp_device() -> HidrawDevice {
    let path = hidpp_node(G502.0, G502.1).expect("no HID++ node found for the G502");
    HidrawDevice::open(&path).expect("data/udev/70-mouser-dev.rules should grant access")
}

#[test]
#[ignore = "requires the G502"]
fn request_round_trips_the_protocol_version() {
    let mut dev = g502_hidpp_device();
    let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);

    // Root feature, getProtocolVersion, with a ping byte to prove the reply
    // is ours and not a notification that happened to arrive.
    const PING: u8 = 0x5a;
    let request = Message::short(DEVICE_INDEX_WIRED, 0x00, 1, &[0x00, 0x00, PING]).unwrap();

    let reply = hidpp.request(&request).expect("the G502 answers a protocol ping");

    assert!(!reply.is_error(), "got an error reply: {reply:02x?}");
    assert_eq!(reply.device_index, DEVICE_INDEX_WIRED);
    assert_eq!(reply.feature_index, 0x00, "from the root feature");
    assert_eq!(reply.function, request.function, "function and swid echoed");
    assert_eq!(reply.params[2], PING, "our ping byte, echoed");

    let (major, minor) = (reply.params[0], reply.params[1]);
    println!("HID++ {major}.{minor}");
    assert!(major >= 2, "expected HID++ 2.0 or later, got {major}.{minor}");
}

#[test]
#[ignore = "requires the G502"]
fn request_accepts_a_long_reply_to_a_short_request() {
    // Observed on every exchange with this device: a 7-byte request is
    // answered with a 20-byte report. `request` must take the length from the
    // reply's own report id, never from the one it sent.
    let mut dev = g502_hidpp_device();
    let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);

    let request = Message::short(DEVICE_INDEX_WIRED, 0x00, 1, &[0, 0, 0x5a]).unwrap();
    assert_eq!(request.params.len(), 3);

    let reply = hidpp.request(&request).unwrap();

    println!("sent {:#04x}, got {:#04x}", request.report_id, reply.report_id);
    assert_eq!(
        reply.params.len(),
        Message::param_len(reply.report_id).unwrap(),
        "the reply's params must match its own report id"
    );
}

#[test]
#[ignore = "requires the G502"]
fn request_surfaces_a_device_error_rather_than_timing_out() {
    // Function 0x0f does not exist on the root feature. The device answers
    //     11 ff ff 00 fa 07 ...
    // where byte 2 is the 2.0 error marker, byte 3 the failed feature index,
    // byte 4 the failed function|swid, and byte 5 error code 7
    // (invalid function id).
    //
    // The point of this test is that an error reply does NOT look like a
    // normal reply — the software id sits in params[0], not in `function`. A
    // matcher that only handles normal replies discards this as a
    // notification and then reports a timeout for a device that answered at
    // once. A 5s guard distinguishes the two failures.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut dev = g502_hidpp_device();
        let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);
        let request = Message::short(DEVICE_INDEX_WIRED, 0x00, 0x0f, &[]).unwrap();
        let _ = tx.send(hidpp.request(&request).map_err(|err| err.to_string()));
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        // `request` is expected to map an error reply to `Err`. If you chose
        // instead to return `Ok` with `is_error()` set, flip these two arms.
        Ok(Ok(reply)) => panic!("expected an error, got {reply:02x?}"),
        Ok(Err(message)) => println!("rejected as: {message}"),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("blocked for 5s on an error the device sent immediately")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("the worker thread panicked; its message is above")
        }
    }
}

#[test]
#[ignore = "requires the G502"]
fn feature_index_resolves_the_features_this_mouse_implements() {
    let mut dev = g502_hidpp_device();
    let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);

    // Indices are per-device and can move with a firmware update, so only
    // non-zero is asserted. For reference, this G502 answered:
    //   FeatureSet 1, FwVersion 3, AdjustableDpi 10, ReportRate 11,
    //   ColorLedEffects 2, OnboardProfiles 12.
    for (feature, name) in [
        (FEATURE_FEATURE_SET, "FeatureSet"),
        (FEATURE_DEVICE_FW_VERSION, "DeviceFwVersion"),
        (FEATURE_ADJUSTABLE_DPI, "AdjustableDpi"),
        (FEATURE_REPORT_RATE, "ReportRate"),
        (FEATURE_COLOR_LED_EFFECTS, "ColorLedEffects"),
        (FEATURE_ONBOARD_PROFILES, "OnboardProfiles"),
    ] {
        let index = hidpp
            .feature_index(feature)
            .unwrap_or_else(|err| panic!("{name} ({feature:#06x}) lookup failed: {err}"))
            .unwrap_or_else(|| panic!("{name} ({feature:#06x}) should be implemented"));

        println!("{name:18} {feature:#06x} -> index {index}");
        assert_ne!(index, 0, "{name}: index 0 means not implemented");
    }
}

#[test]
#[ignore = "requires the G502"]
fn feature_index_is_none_for_a_feature_the_device_lacks() {
    let mut dev = g502_hidpp_device();
    let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);

    // Both answer `11 ff 00 0a 00 00 00` — index 0, meaning not implemented.
    // 0x1b04 is real (SpecialKeysMSEButtons) and this mouse does not have it;
    // 0xdead is not a feature at all. Neither is an error: the device
    // answered correctly, it just does not implement them.
    for feature in [0x1b04u16, 0x0020, 0xdead] {
        assert_eq!(
            hidpp.feature_index(feature).unwrap(),
            None,
            "{feature:#06x} should resolve to None, not an error"
        );
    }
}

#[test]
#[ignore = "requires the G502"]
fn the_root_feature_resolves_to_index_zero() {
    // A wart worth pinning. `getFeature(0x0000)` answers index 0 version 1,
    // while an unimplemented feature answers index 0 version 0 — so by index
    // alone the root feature is indistinguishable from absent.
    //
    // The root feature is *defined* to live at index 0; it is the fixed point
    // the rest of the protocol bootstraps from. So `feature_index` should
    // short-circuit it rather than ask the device. This asserts that
    // short-circuit; if you document the precondition instead of coding it,
    // expect `Ok(None)` here and say so.
    let mut dev = g502_hidpp_device();
    let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);

    assert_eq!(hidpp.feature_index(FEATURE_ROOT).unwrap(), Some(0));
}

#[test]
#[ignore = "requires the G502"]
fn request_fails_rather_than_hanging_on_a_node_that_never_answers() {
    // The G502's other node is the boot-mouse interface. Its report
    // descriptor declares no report ids at all, so it cannot carry a 0x10
    // report — but `write(2)` to it succeeds anyway and no reply ever comes.
    //
    // This is the test that forces the timeout. Without one, `request` sits
    // in a blocking `read(2)` forever and this would hang the suite rather
    // than fail it; the thread and the 5s guard turn that into a message.
    //
    // It is also how the hidpp20 driver rejects the wrong node at probe time,
    // since bus/vid/pid are identical on both and the database cannot tell
    // them apart.
    let hidpp = hidpp_node(G502.0, G502.1).expect("no HID++ node found");
    let plain = nodes_for(G502.0, G502.1)
        .into_iter()
        .find(|path| *path != hidpp)
        .expect("the G502 should present a second, non-HID++ node");

    println!("probing {} (expected to be silent)", plain.display());

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = HidrawDevice::open(&plain).and_then(|mut dev| {
            let mut hidpp = Hidpp20::new(&mut dev, DEVICE_INDEX_WIRED);
            let request = Message::short(DEVICE_INDEX_WIRED, 0x00, 1, &[0, 0, 0x5a])?;
            hidpp.request(&request)
        });
        let _ = tx.send(result.map_err(|err| err.to_string()));
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Err(message)) => println!("gave up as: {message}"),
        Ok(Ok(reply)) => panic!("the boot-mouse interface should not speak HID++: {reply:02x?}"),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!(
            "`request` blocked for 5s — it needs a deadline, and \
             `HidrawDevice` needs a poll-based read to give it one"
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("the worker thread panicked; its message is above")
        }
    }
}

// ====================================================== Zelotes C-18 ======

/// Vendor and product of the Zelotes C-18 vertical mouse — a SOAI rebadge.
const C18: (u16, u16) = (0x12c9, 0x2003);

/// A node's USB vendor and product, read from sysfs.
///
/// Unlike `nodes_for`, this never opens the device node, so it works on a
/// device the udev rule does not cover yet. `syspath/device` is the HID
/// device; two levels above it is the USB device carrying the ids.
fn usb_ids(syspath: &Path) -> Option<(u16, u16)> {
    let read = |name: &str| {
        std::fs::read_to_string(syspath.join("device/../..").join(name))
            .ok()
            .and_then(|text| u16::from_str_radix(text.trim(), 16).ok())
    };
    Some((read("idVendor")?, read("idProduct")?))
}

/// True when the descriptor declares the C-18's configuration collection.
///
/// `06 00 ff` Usage Page (Vendor-Defined 0xFF00), `95 40` Report Count (64),
/// `91 02` Output, `b1 02` Feature. The boot-mouse and keyboard interfaces
/// have none of these.
fn speaks_c18_config(descriptor: &[u8]) -> bool {
    let has = |needle: &[u8]| descriptor.windows(needle.len()).any(|w| w == needle);
    has(&[0x06, 0x00, 0xff]) && has(&[0x95, 0x40]) && has(&[0x91, 0x02]) && has(&[0xb1, 0x02])
}

/// Every C-18 node with its report descriptor, found without opening a node.
fn c18_descriptors() -> Vec<(PathBuf, Vec<u8>)> {
    enumerate()
        .expect("enumerate should work on a live system")
        .into_iter()
        .filter(|node| usb_ids(&node.syspath) == Some(C18))
        .filter_map(|node| report_descriptor(&node.syspath).map(|d| (node.devnode, d)))
        .collect()
}

#[test]
#[ignore = "requires the Zelotes C-18"]
fn the_c18_presents_three_nodes() {
    // devices/zelotes-c18.device documents three interfaces. All three carry
    // the same bus/vid/pid, so `DeviceMatch=` cannot separate them and
    // probing has to.
    let nodes = c18_descriptors();

    assert!(
        !nodes.is_empty(),
        "no {:04x}:{:04x} nodes found — is the C-18 plugged in?",
        C18.0,
        C18.1
    );
    for (devnode, _) in &nodes {
        println!("{}", devnode.display());
    }
    assert_eq!(nodes.len(), 3, "expected three nodes, found {}", nodes.len());
}

#[test]
#[ignore = "requires the Zelotes C-18"]
fn exactly_one_c18_node_is_the_configuration_interface() {
    let nodes = c18_descriptors();
    assert!(!nodes.is_empty(), "the C-18 was not found");

    let config: Vec<&PathBuf> = nodes
        .iter()
        .filter(|(_, descriptor)| speaks_c18_config(descriptor))
        .map(|(devnode, _)| devnode)
        .collect();

    assert_eq!(
        config.len(),
        1,
        "expected one configuration interface, found {config:?}"
    );
    println!("config interface: {}", config[0].display());
}

#[test]
#[ignore = "requires the Zelotes C-18"]
fn the_c18_does_not_speak_hidpp() {
    // The justification for a separate driver. The C-18's vendor collection
    // carries 64-byte reports with no report id; HID++ needs report 0x10 or
    // 0x11 declared. Were this to pass, the device would belong on hidpp20
    // and the `c18` driver would not need writing.
    for (devnode, descriptor) in c18_descriptors() {
        assert!(
            !speaks_hidpp(&descriptor),
            "{} declares a HID++ collection",
            devnode.display()
        );
    }
}

#[test]
#[ignore = "requires the Zelotes C-18 and the udev rule"]
fn the_c18_configuration_interface_can_be_opened() {
    // Separated from the descriptor tests above, which read sysfs and need no
    // permission. This one needs data/udev/70-mouser-dev.rules installed, so
    // its failure means "install the rule", not "the device file is wrong".
    let (devnode, _) = c18_descriptors()
        .into_iter()
        .find(|(_, descriptor)| speaks_c18_config(descriptor))
        .expect("no C-18 configuration interface found");

    let dev = HidrawDevice::open(&devnode).unwrap_or_else(|err| {
        panic!(
            "cannot open {}: {err}\n  install the dev rule:\n    \
             sudo install -m 644 data/udev/70-mouser-dev.rules /etc/udev/rules.d/\n    \
             sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw",
            devnode.display()
        )
    });

    let info = dev.info();
    println!("{:04x}:{:04x} {:?} at {:?}", info.vendor, info.product, info.name, info.phys);

    assert_eq!((info.vendor, info.product), C18);
    assert_eq!(info.bus, BusType::Usb);
}

/// Open the C-18's configuration interface, ready for a transaction.
///
/// Found by report descriptor, never by node name: this mouse has been
/// `hidraw14` and `hidraw12` on the same machine.
fn c18_config_device() -> HidrawDevice {
    let (devnode, _) = c18_descriptors()
        .into_iter()
        .find(|(_, descriptor)| speaks_c18_config(descriptor))
        .expect("no C-18 configuration interface found — is it plugged in?");

    HidrawDevice::open(&devnode).unwrap_or_else(|err| {
        panic!(
            "cannot open {}: {err}\n  install the dev rule:\n    \
             sudo install -m 644 data/udev/70-mouser-dev.rules /etc/udev/rules.d/",
            devnode.display()
        )
    })
}

#[test]
#[ignore = "requires the Zelotes C-18, and CHANGES its active DPI stage"]
fn the_mouse_accepts_a_dpi_stage_transaction() {
    // The first thing this project writes to hardware. Three 8-byte Feature
    // reports and nothing else — no payloads, no tables.
    //
    // There is no readback on this device, so passing only proves the writes
    // were accepted: that the Feature-report path reaches the right
    // interface, that the checksums are right, and that the 1-based profile
    // numbering is right. Whether the mouse *obeyed* is something you have to
    // feel — watch the cursor speed as it runs with --nocapture.
    //
    // It leaves the mouse on stage 3. Your DPI button cycles it back.
    let mut dev = c18_config_device();
    let mut c18 = C18::new(&mut dev);

    for stage in [1u8, NUM_RESOLUTIONS as u8, 3] {
        eprintln!("profile 1, DPI stage {stage} of {NUM_RESOLUTIONS}");

        c18.begin().expect("begin");
        c18.send_command(Command::new(OP_SET_DPI_STAGE, 1, stage))
            .unwrap_or_else(|err| panic!("set stage {stage}: {err}"));
        c18.select_and_commit(0).expect("select and commit");

        std::thread::sleep(Duration::from_millis(600));
    }
}

#[test]
#[ignore = "requires the Zelotes C-18"]
fn a_command_to_the_wrong_interface_is_rejected() {
    // The negative control, and the only automated evidence that the writes
    // above genuinely reach hardware rather than being swallowed somewhere.
    //
    // Interfaces 0 and 1 declare no vendor 0xFF00 collection and no 8-byte
    // Feature report, so the device stalls the control transfer — upstream
    // saw EPIPE here. If this passed on every interface, the config writes
    // would be going nowhere and the test above would pass regardless.
    let wrong: Vec<PathBuf> = c18_descriptors()
        .into_iter()
        .filter(|(_, descriptor)| !speaks_c18_config(descriptor))
        .map(|(devnode, _)| devnode)
        .collect();

    assert_eq!(
        wrong.len(),
        2,
        "expected two non-config interfaces, found {wrong:?}"
    );

    for devnode in wrong {
        let Ok(mut dev) = HidrawDevice::open(&devnode) else {
            eprintln!("{}: no permission, skipping", devnode.display());
            continue;
        };

        let result = C18::new(&mut dev).begin();
        eprintln!("{}: {result:?}", devnode.display());

        assert!(
            result.is_err(),
            "{} accepted a C18 command but declares no config collection",
            devnode.display()
        );
    }
}

// ============================================== report descriptors ========

#[test]
#[ignore = "requires real hardware"]
fn the_report_descriptor_ioctl_agrees_with_sysfs() {
    // Two independent paths to the same bytes: `HIDIOCGRDESCSIZE` +
    // `HIDIOCGRDESC` through the device node, versus
    // `device/report_descriptor` in sysfs. If they agree, both the ioctl pair
    // and the length slicing are right — neither could be checked against the
    // other inside the library.
    //
    // This is the test that would catch slicing by the whole 4096-byte array,
    // or by `HidrawReportDescriptor::size` after the call, or forgetting to
    // set `size` before it.
    let mut checked = 0;

    for node in enumerate().expect("enumerate") {
        let Some(from_sysfs) = report_descriptor(&node.syspath) else {
            continue; // sysfs entry going away mid-scan
        };
        let Ok(dev) = HidrawDevice::open(&node.devnode) else {
            continue; // no permission for this one; not what we are testing
        };

        let from_ioctl = dev
            .report_descriptor()
            .unwrap_or_else(|err| panic!("{}: {err}", node.devnode.display()));

        assert_eq!(
            from_ioctl.len(),
            from_sysfs.len(),
            "{}: ioctl gave {} bytes, sysfs {}",
            node.devnode.display(),
            from_ioctl.len(),
            from_sysfs.len()
        );
        assert_eq!(
            from_ioctl,
            from_sysfs,
            "{} descriptors differ\n  ioctl: {:02x?}\n  sysfs: {:02x?}",
            node.devnode.display(),
            from_ioctl,
            from_sysfs
        );

        println!("{}: {} bytes agree", node.devnode.display(), from_ioctl.len());
        checked += 1;
    }

    assert!(
        checked >= 2,
        "only {checked} nodes were readable — install the dev rule, or this \
         proves very little"
    );
}

#[test]
#[ignore = "requires real hardware"]
fn descriptors_are_never_empty_and_fit_the_kernel_limit() {
    for node in enumerate().expect("enumerate") {
        let Ok(dev) = HidrawDevice::open(&node.devnode) else {
            continue;
        };

        let descriptor = dev.report_descriptor().expect("report_descriptor");

        // A HID device without a report descriptor cannot exist: the kernel
        // needs one to have bound hid-generic in the first place.
        assert!(
            !descriptor.is_empty(),
            "{} returned an empty descriptor",
            node.devnode.display()
        );
        assert!(
            descriptor.len() <= HID_MAX_DESCRIPTOR_SIZE,
            "{} returned {} bytes, over the kernel's limit",
            node.devnode.display(),
            descriptor.len()
        );
    }
}

#[test]
#[ignore = "requires the Zelotes C-18"]
fn the_c18_config_interface_is_identifiable_through_the_ioctl() {
    // Closes the loop for `C18::speaks_c18`, which has to work from the
    // ioctl rather than from sysfs — a `HidrawDevice` keeps no syspath.
    //
    // Until now every descriptor check in this file read sysfs, so the
    // predicates were never exercised against the bytes the driver will
    // actually see.
    let dev = c18_config_device();
    let descriptor = dev.report_descriptor().expect("report_descriptor");

    println!("{} bytes: {:02x?}", descriptor.len(), descriptor);

    assert!(
        speaks_c18_config(&descriptor),
        "the config interface should satisfy the C18 predicate"
    );
    assert!(
        !speaks_hidpp(&descriptor),
        "the C-18 must not look like a HID++ device"
    );
}

#[test]
#[ignore = "requires the G502"]
fn the_g502_hidpp_node_is_identifiable_through_the_ioctl() {
    // The same loop closed for `Hidpp20Driver::probe`.
    let dev = g502_hidpp_device();
    let descriptor = dev.report_descriptor().expect("report_descriptor");

    println!("{} bytes: {:02x?}", descriptor.len(), descriptor);

    assert!(
        speaks_hidpp(&descriptor),
        "the vendor node should satisfy the HID++ predicate"
    );
    assert!(
        !speaks_c18_config(&descriptor),
        "a HID++ device must not look like a C-18"
    );
}
