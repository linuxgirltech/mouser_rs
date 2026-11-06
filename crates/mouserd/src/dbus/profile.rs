//! `org.freedesktop.mouser1.Profile` — one stored configuration.

use zbus::OwnedObjectPath;
use zbus::interface;

use crate::state::SharedDevice;

pub struct ProfileIface {
    pub device: SharedDevice,
    pub index: u32,
    pub resolution_paths: Vec<OwnedObjectPath>,
    pub button_paths: Vec<OwnedObjectPath>,
    pub led_paths: Vec<OwnedObjectPath>,
}

#[interface(name = "org.freedesktop.mouser1.Profile")]
impl ProfileIface {
    /// `u`, read-only, constant. Zero-based.
    #[zbus(property)]
    async fn index(&self) -> u32 {
        self.index
    }

    /// `s`, read-write. An empty name means the name cannot be changed, so
    /// the setter should refuse with `fdo::Error::PropertyReadOnly` in that
    /// case rather than silently accepting.
    #[zbus(property)]
    async fn name(&self) -> String {
        todo!()
    }

    #[zbus(property)]
    async fn set_name(&mut self, value: String) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `b`, read-write. A disabled profile may hold invalid bindings, so
    /// rebind everything before committing one.
    #[zbus(property)]
    async fn disabled(&self) -> bool {
        todo!()
    }

    #[zbus(property)]
    async fn set_disabled(&mut self, value: bool) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `b`, read-only. Use [`ProfileIface::set_active`] to change it: a
    /// profile can only be set active, never inactive, because one must
    /// always be active.
    #[zbus(property, name = "IsActive")]
    async fn is_active(&self) -> bool {
        todo!()
    }

    /// `b`, read-only. True once any other property of this profile has been
    /// written and not yet committed.
    #[zbus(property, name = "IsDirty")]
    async fn is_dirty(&self) -> bool {
        todo!()
    }

    /// `ao`, read-only. Mutable only where resolutions can be added/removed.
    #[zbus(property)]
    async fn resolutions(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }

    /// `ao`, read-only, constant.
    #[zbus(property)]
    async fn buttons(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }

    /// `ao`, read-only, constant.
    #[zbus(property)]
    async fn leds(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }

    /// `i`, read-write. `1`/`0`, or `-1` when the device can neither read nor
    /// write it — in which case reject writes.
    #[zbus(property, name = "AngleSnapping")]
    async fn angle_snapping(&self) -> i32 {
        todo!()
    }

    #[zbus(property, name = "AngleSnapping")]
    async fn set_angle_snapping(&mut self, value: i32) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `i`, read-write. Milliseconds, and must be one of
    /// [`ProfileIface::debounces`]; `-1` means not settable.
    #[zbus(property)]
    async fn debounce(&self) -> i32 {
        todo!()
    }

    #[zbus(property)]
    async fn set_debounce(&mut self, value: i32) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `au`, constant, ascending. Empty when the device cannot report or set
    /// a debounce time.
    ///
    /// The published API marks this "read-write, constant", which is
    /// self-contradictory for a list of permitted values — it is exported
    /// read-only here. Revisit if a real client turns out to write it.
    #[zbus(property)]
    async fn debounces(&self) -> Vec<u32> {
        todo!()
    }

    /// `u`, read-write. Hz, and must be one of
    /// [`ProfileIface::report_rates`].
    #[zbus(property, name = "ReportRate")]
    async fn report_rate(&self) -> u32 {
        todo!()
    }

    #[zbus(property, name = "ReportRate")]
    async fn set_report_rate(&mut self, value: u32) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `au`, constant, ascending. Empty when unsupported. Same API wording
    /// problem as [`ProfileIface::debounces`].
    #[zbus(property, name = "ReportRates")]
    async fn report_rates(&self) -> Vec<u32> {
        todo!()
    }

    /// Make this the active profile.
    async fn set_active(&self) -> zbus::fdo::Result<()> {
        todo!()
    }
}
