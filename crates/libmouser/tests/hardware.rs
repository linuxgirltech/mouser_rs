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

use libmouser::enumerate::{HotplugEvent, Monitor, enumerate};
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
        Err(_) => panic!("no reply within 2s — wrong node, or the device is asleep"),
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
    let mut monitor = Monitor::new().expect("monitor should open");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut seen: Vec<(HotplugEvent, PathBuf)> = Vec::new();

    eprintln!("unplug the mouse, then plug it back in...");
    while std::time::Instant::now() < deadline && seen.len() < 2 {
        while let Some((kind, node)) = monitor.poll_event() {
            eprintln!("{kind:?} {}", node.devnode.display());
            seen.push((kind, node.devnode));
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(!seen.is_empty(), "no hotplug events in 30s — did you unplug it?");
    assert!(
        seen.iter().any(|(k, _)| *k == HotplugEvent::Removed),
        "expected a Removed event, got {seen:?}"
    );
    assert!(
        seen.iter().any(|(k, _)| *k == HotplugEvent::Added),
        "expected an Added event, got {seen:?}"
    );
}