//! Shared state between the D-Bus objects.
//!
//! Each device is behind its own lock, so one device committing (which can
//! take seconds on real hardware) does not block reads of another. The
//! manager's device list is a separate lock, held only briefly.

use std::collections::BTreeMap;
use std::sync::Arc;

use libmouser::ManagedDevice;
use tokio::sync::RwLock;
use zbus::OwnedObjectPath;

/// One device, shared by its Device/Profile/Resolution/Button/Led objects.
pub type SharedDevice = Arc<RwLock<ManagedDevice>>;

/// What the daemon knows about one registered device.
pub struct Registration {
    /// The object-path id, e.g. `hidraw3`.
    pub id: String,
    pub device: SharedDevice,
    /// This device's own object path.
    pub path: OwnedObjectPath,
}

/// The manager's view: every device currently exported.
#[derive(Default)]
pub struct State {
    devices: BTreeMap<String, Registration>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// The `ao` value for `Manager.Devices`.
    pub fn device_paths(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }

    pub fn insert(&mut self, registration: Registration) {
        todo!()
    }

    pub fn remove(&mut self, id: &str) -> Option<Registration> {
        todo!()
    }

    pub fn get(&self, id: &str) -> Option<&Registration> {
        todo!()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Registration> + '_ {
        self.devices.values()
    }
}

/// The manager's shared handle.
pub type SharedState = Arc<RwLock<State>>;
