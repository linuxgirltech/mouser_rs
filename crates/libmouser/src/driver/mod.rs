//! Driver interface.
//!
//! A driver does two things: build a [`Device`] from hardware at probe time,
//! and write a modified one back on commit. Probing is stateless and lives on
//! [`Driver`]; anything that needs to hold an open fd lives on the
//! [`DeviceBackend`] probing returns.
//!
//! To add a driver: write the module, implement both traits, and register it
//! in [`DriverRegistry::with_builtin`].
//!
//! Drivers libratbag ships, by share of its 113 shipped device files —
//! `hidpp20` (58), `asus` (16), `hidpp10` (13), `steelseries` (10),
//! `sinowealth` (6), then one each of `roccat`, `roccat-kone-pure`,
//! `roccat-kone-emp`, `logitech_g300`, `logitech_g600`, `etekcity`, `gskill`,
//! `marsgaming`, `openinput`, `sinowealth_nubwo`.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::db::DeviceEntry;
use crate::device::Device;
use crate::error::{Result, Error};
use crate::hid::HidrawDevice;

pub mod hidpp20;

/// Everything a driver may look at while probing. A driver may talk to the
/// node here — feature discovery usually happens at probe time.
pub struct ProbeContext<'a> {
    /// The database entry that matched this hardware.
    pub entry: &'a DeviceEntry,
    pub hidraw: &'a mut HidrawDevice,
}

/// Probing produced a usable device.
pub struct ProbeResult {
    pub device: Device,
    pub backend: Box<dyn DeviceBackend>,
}

/// Builds devices from hardware.
///
/// One instance is shared across every device it handles, so implementations
/// must hold no per-device state — that is what [`DeviceBackend`] is for.
pub trait Driver: Send + Sync + 'static {
    /// The name used in `Driver=` lines.
    fn id(&self) -> &'static str;

    /// Read the device's current configuration and describe it.
    fn probe(&self, ctx: ProbeContext<'_>) -> Result<ProbeResult>;
}

/// Per-device state, and the writes.
pub trait DeviceBackend: Send + Sync {
    /// Write every staged change to the hardware.
    ///
    /// On success the caller clears the dirty flags. On failure the daemon
    /// emits `Resync` and re-reads through [`DeviceBackend::reload`].
    fn commit(&mut self, device: &mut Device, hidraw: &mut HidrawDevice) -> Result<()>;

    /// Re-read the hardware into `device`, discarding staged changes.
    ///
    /// The default is a no-op, which is right for devices whose state
    /// libmouser fully owns.
    fn reload(&mut self, device: &mut Device, hidraw: &mut HidrawDevice) -> Result<()> {
        let _ = (device, hidraw);
        Ok(())
    }

    /// Switch profile now rather than at commit, for devices that support it.
    /// The default just stages it like any other change.
    fn set_active_profile(&mut self, device: &mut Device, hidraw: &mut HidrawDevice, index: u32) -> Result<()> {
        let _ = hidraw;
        device.set_active_profile(index)
    }
}

/// The drivers this build knows about, keyed by `Driver=` name.
#[derive(Clone, Default)]
pub struct DriverRegistry {
    drivers: BTreeMap<&'static str, Arc<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every driver that is implemented and ready to use.
    ///
    /// Empty for now — register each driver here as you finish it. A driver
    /// absent from this map surfaces as `Error::UnknownDriver` when a
    /// matching device turns up, which is the intended behaviour.
    pub fn with_builtin() -> Self {
        Self::new()
    }

    pub fn register(&mut self, driver: Arc<dyn Driver>) {
        self.drivers.insert(driver.id(), driver);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Driver>> {
        self.drivers.get(id).cloned()
    }

    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.drivers.keys().copied()
    }

    /// Probe `hidraw` with whichever driver `entry` names.
    pub fn probe(&self, entry: &DeviceEntry, hidraw: &mut HidrawDevice) -> Result<ProbeResult> {
        let driver = self
            .get(&entry.driver)
            .ok_or_else(|| Error::UnknownDriver(entry.driver.clone()))?;

        driver.probe(ProbeContext {entry, hidraw})
    }
}

impl std::fmt::Debug for DriverRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriverRegistry")
            .field("drivers", &self.drivers.keys().collect::<Vec<_>>())
            .finish()
    }
}
