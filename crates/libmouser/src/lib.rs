//! libmouser — configuration of gaming mice and other configurable input
//! devices. A Rust reimplementation of libratbag.
//!
//! The device model and value types follow the published `ratbagd` D-Bus API
//! closely, so the daemon on top stays a translation layer rather than a
//! second model.
//!
//! ```text
//! Context ── DeviceDb          parsed .device files, matched on bus/vid/pid
//!         ├─ DriverRegistry    Driver=... → impl Driver
//!         └─ Vec<ManagedDevice>
//!                 ├─ Device    profiles → resolutions / buttons / LEDs
//!                 └─ Box<dyn DeviceBackend>   holds the fd, writes on commit
//! ```
//!
//! Changing a setting is three steps, matching the D-Bus API: mutate the
//! [`Device`], call [`ManagedDevice::commit`], and on failure re-read with
//! [`ManagedDevice::reload`] — the point at which the daemon signals
//! `Resync`.
//!
//! # Status
//!
//! Skeleton. Every behavioural body is `todo!()`; the wire constants, struct
//! shapes and trait signatures are in place. No driver is implemented yet, so
//! [`Context::probe_all`] finds nothing.

// TODO: delete once the `todo!()` bodies are filled in — they make every
// parameter look unused.
#![allow(unused_variables, dead_code)]

pub mod db;
pub mod device;
pub mod driver;
pub mod error;
pub mod hid;
pub mod types;

#[cfg(feature = "udev")]
pub mod enumerate;

pub use db::{DeviceDb, DeviceEntry};
pub use device::{Button, Device, Led, Profile, Resolution};
pub use driver::{DeviceBackend, Driver, DriverRegistry, ProbeResult};
pub use error::{Error, Result};
pub use hid::{HidInfo, HidrawDevice};
pub use types::{
    ActionType, BusType, ButtonAction, Color, ColorDepth, DeviceType, Dpi, LedMode, MacroEvent,
    ModelId, ResolutionCaps, SpecialAction,
};

use std::path::{Path, PathBuf};

/// A probed device: its configuration, and the backend that writes it.
pub struct ManagedDevice {
    device: Device,
    backend: Box<dyn DeviceBackend>,
    hidraw: HidrawDevice,
}

impl ManagedDevice {
    pub fn from_probe(result: ProbeResult, hidraw: HidrawDevice) -> Self {
        Self {
            device: result.device,
            backend: result.backend,
            hidraw,
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Mutable access. Callers set `is_dirty` on the profiles they touch —
    /// the `set_*` helpers on [`Profile`] do it for you.
    pub fn device_mut(&mut self) -> &mut Device {
        &mut self.device
    }

    /// Write staged changes and clear the dirty flags.
    ///
    /// On failure leave the flags set, so the caller can retry or resync.
    pub fn commit(&mut self) -> Result<()> {
        self.backend.commit(&mut self.device, &mut self.hidraw)
    }

    /// Re-read the hardware, discarding staged changes.
    pub fn reload(&mut self) -> Result<()> {
        self.backend.commit(&mut self.device, &mut self.hidraw)
    }

    /// Switch profiles, letting the backend do it immediately if it can.
    pub fn set_active_profile(&mut self, index: u32) -> Result<()> {
        self.backend.set_active_profile(&mut self.device, &mut self.hidraw, index)
    }
}

impl std::fmt::Debug for ManagedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedDevice")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

/// Owns the database, the drivers, and every device probed so far.
pub struct Context {
    db: DeviceDb,
    registry: DriverRegistry,
    devices: Vec<ManagedDevice>,
}

impl Context {
    pub fn new(db: DeviceDb, registry: DriverRegistry) -> Self {
        Self {
            db,
            registry,
            devices: Vec::new(),
        }
    }

    /// Load the database from `dir` and register the built-in drivers.
    pub fn with_data_dir(dir: impl AsRef<Path>) -> Result<Self> {
        todo!()
    }

    pub fn db(&self) -> &DeviceDb {
        &self.db
    }

    pub fn registry(&self) -> &DriverRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DriverRegistry {
        &mut self.registry
    }

    pub fn devices(&self) -> &[ManagedDevice] {
        &self.devices
    }

    pub fn devices_mut(&mut self) -> &mut [ManagedDevice] {
        &mut self.devices
    }

    /// Add an already-probed device.
    pub fn push_device(&mut self, device: ManagedDevice) {
        self.devices.push(device);
    }

    /// Open a hidraw node, match it against the database, and probe it.
    ///
    /// Returns the index of the new device in [`Context::devices`].
    /// [`Error::UnknownDevice`] means this is not a device we configure — the
    /// normal outcome for most hidraw nodes on a system.
    pub fn add_hidraw(&mut self, path: impl AsRef<Path>) -> Result<usize> {
        let path = path.as_ref();
        let mut hidraw = HidrawDevice::open(path)?;

        // Copy the IDs out: `info()` borrows `hidraw`, and `probe` needs it
        // mutably a few lines down.
        let info = hidraw.info();
        let (bus, vendor, product) = (info.bus, info.vendor, info.product);

        let entry = self.db.find(bus, vendor, product).ok_or_else(|| {
            Error::UnknownDevice(ModelId::new(bus, vendor, product).to_string())
        })?;

        let mut result = self.registry.probe(entry, &mut hidraw)?;
        result.device.devnode = Some(path.to_path_buf());

        let index = self.devices.len();
        self.devices.push(ManagedDevice::from_probe(result, hidraw));
        Ok(index)
    }

    /// Drop the device probed from `devnode`. Returns whether one went.
    pub fn remove_devnode(&mut self, devnode: &Path) -> bool {
        todo!()
    }

    /// Probe every hidraw node on the system, returning how many were added.
    ///
    /// Unclaimed nodes are skipped quietly: `UnknownDevice` is not worth a
    /// log line, `UnknownDriver` and `ProbeFailed` are worth one at debug or
    /// info, anything else at warn.
    #[cfg(feature = "udev")]
    pub fn probe_all(&mut self) -> Result<usize> {
        let nodes = crate::enumerate::enumerate()?;
        let mut added = 0usize;

        for node in nodes {
            if self.devices.iter().any(|d| d.device.devnode.as_deref() == Some(&*node.devnode)) {
                continue;
            }

            match self.add_hidraw(&node.devnode) {
                Ok(_) => added += 1,
                Err(err @ Error::UnknownDevice(_)) => {
                    tracing::trace!(node = %node.devnode.display(), %err);
                }
                Err(err @ (Error::UnknownDriver(_) | Error::ProbeFailed { .. })) => {
                    tracing::debug!(node = %node.devnode.display(), %err, "Node not claimed");
                }
                Err(err) => {
                    tracing::warn!(node = %node.devnode.display(), %err, "Probe failed");
                }
            }
        }

        Ok(added)
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("db_entries", &self.db.entries().len())
            .field("drivers", &self.registry.ids().collect::<Vec<_>>())
            .field("devices", &self.devices.len())
            .finish()
    }
}

/// Where the database lives: `$LIBMOUSER_DATA_DIR`, else the compiled-in path.
pub fn default_data_dir() -> PathBuf {
    std::env::var_os("LIBMOUSER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(db::DEFAULT_DATA_DIR))
}
