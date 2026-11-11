//! Finding hidraw nodes, and watching for new ones. Requires the `udev`
//! feature; without it, feed paths in via `Context::add_hidraw`.

use std::path::PathBuf;

use crate::error::Result;

/// A candidate node, before any driver has looked at it.
#[derive(Debug, Clone)]
pub struct HidrawNode {
    /// `/dev/hidraw3`
    pub devnode: PathBuf,
    /// `/sys/devices/.../hidraw/hidraw3`
    pub syspath: PathBuf,
}

/// Build a [`HidrawNode`] from a udev device.
///
/// `None` when the device has no `/dev` node — a sysfs entry being torn down
/// as we scan, or one whose node has not been created yet. Shared with
/// [`Monitor::poll_event`], which needs the identical conversion.
fn node_from_device(device: &udev::Device) -> Option<HidrawNode> {
    Some(HidrawNode { 
        devnode: device.devnode()?.to_path_buf(), 
        syspath: device.syspath().to_path_buf() 
    })
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
        .filter_map(|device| node_from_device(&device))
        .collect();

    nodes.sort_by(|a, b| a.devnode.cmp(&b.devnode));
    Ok(nodes)
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
    socket: udev::MonitorSocket,
}

impl Monitor {
    pub fn new() -> Result<Self> {
        let socket = udev::MonitorBuilder::new()?
            .match_subsystem("hidraw")?
            .listen()?;

        Ok(Self { socket })
    }

    /// Take the next pending event, if any. Must not block.
    ///
    /// Only `Add` and `Remove` matter — `change`/`bind`/`unbind` do not alter
    /// which nodes exist, so skip them and keep draining.
    pub fn poll_event(&mut self) -> Option<(HotplugEvent, HidrawNode)> {
        loop {
            let event = self.socket.iter().next()?;

            let kind = match event.event_type() {
                udev::EventType::Add => HotplugEvent::Added,
                udev::EventType::Remove => HotplugEvent::Removed,
                // change/bind/unbind do not alter which node exist
                _ => continue,
            };

            if let Some(node) = node_from_device(&event) {
                return Some((kind, node));
            }
        }
    }
}

impl std::os::fd::AsRawFd for Monitor {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.socket.as_raw_fd()
    }
}
