//! Finding hidraw nodes, and watching for new ones. Requires the `udev`
//! feature; without it, feed paths in via `Context::add_hidraw`.

use std::path::PathBuf;

use crate::{HidrawDevice, error::Result};

/// A candidate node, before any driver has looked at it.
#[derive(Debug, Clone)]
pub struct HidrawNode {
    /// `/dev/hidraw3`
    pub devnode: PathBuf,
    /// `/sys/devices/.../hidraw/hidraw3`
    pub syspath: PathBuf,
}

/// List every hidraw node currently present.
///
/// Match subsystem `hidraw` and skip entries with no devnode. Sorting by
/// devnode keeps object paths stable across restarts.
pub fn enumerate() -> Result<Vec<HidrawNode>> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("hidraw")?;
    enumerator.match_is_initialized()?;

    let mut nodes: Vec<HidrawNode> = enumerator
        .scan_devices()?
        .filter_map(|device| {
            Some(HidrawNode {
                devnode: device.devnode()?.to_path_buf(),
                syspath: device.syspath().to_path_buf()
            })
        })
        .collect();

    nodes.sort_by(|a, b| a.devnode.cmp(&b.devnode));
    Ok(nodes)
}

#[test]
#[ignore = "requires real hardware"]
fn list_the_actual_nodes() {
    for node in crate::enumerate::enumerate().unwrap() {
        let info = crate::HidrawDevice::open(&node.devnode)
            .map(|dev| format!("{:?}", dev.info()))
            .unwrap_or_else(|err| format!("<{err}>"));
        println!("{} - {info}", node.devnode.display())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugEvent {
    Added,
    Removed,
}

/// A udev monitor over the `hidraw` subsystem.
///
/// The socket becomes readable when an event is pending, so the daemon
/// registers the fd with its reactor (`tokio::io::unix::AsyncFd`) and drains
/// [`Monitor::poll_event`] when it fires.
pub struct Monitor {
    // TODO: hold the `udev::MonitorSocket` here.
    _private: (),
}

impl Monitor {
    pub fn new() -> Result<Self> {
        todo!("MonitorBuilder::new()?.match_subsystem(\"hidraw\")?.listen()")
    }

    /// Take the next pending event, if any. Must not block.
    ///
    /// Only `Add` and `Remove` matter — `change`/`bind`/`unbind` do not alter
    /// which nodes exist, so skip them and keep draining.
    pub fn poll_event(&mut self) -> Option<(HotplugEvent, HidrawNode)> {
        todo!()
    }
}

impl std::os::fd::AsRawFd for Monitor {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        todo!()
    }
}
