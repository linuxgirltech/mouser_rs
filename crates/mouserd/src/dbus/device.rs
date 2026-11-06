//! `org.freedesktop.mouser1.Device` — one device.

use zbus::OwnedObjectPath;
use zbus::interface;
use zbus::object_server::SignalEmitter;

use crate::state::SharedDevice;

pub struct DeviceIface {
    pub device: SharedDevice,
    /// Precomputed at registration: the `ao` for `Profiles`.
    pub profile_paths: Vec<OwnedObjectPath>,
}

impl DeviceIface {
    pub fn new(device: SharedDevice, profile_paths: Vec<OwnedObjectPath>) -> Self {
        Self {
            device,
            profile_paths,
        }
    }
}

#[interface(name = "org.freedesktop.mouser1.Device")]
impl DeviceIface {
    /// `s`, read-only, constant. `usb:046d:c08b:0`, `bluetooth:…`, or
    /// `unknown`. A device whose model cannot be determined is a bug in the
    /// driver, not an expected state.
    #[zbus(property)]
    async fn model(&self) -> String {
        todo!()
    }

    /// `s`, read-only, constant. Presentable name.
    #[zbus(property)]
    async fn name(&self) -> String {
        todo!()
    }

    /// `s`, read-only, constant. Empty string when the device reports none.
    /// Formatting a major/minor or numeric version is driver-defined.
    #[zbus(property, name = "FirmwareVersion")]
    async fn firmware_version(&self) -> String {
        todo!()
    }

    /// `ao`, read-only. Mutable only on devices that can add and remove
    /// profiles.
    #[zbus(property)]
    async fn profiles(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }

    /// Commit staged changes.
    ///
    /// **Always succeeds.** The write happens asynchronously; spawn it and
    /// return. On failure, emit [`DeviceIface::resync`] and refresh every
    /// property from the device's real state.
    async fn commit(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// Emitted when writing to the device fails. Clients resync their
    /// property values on receipt — expect to send this several seconds
    /// after `Commit()` returned.
    #[zbus(signal)]
    async fn resync(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
