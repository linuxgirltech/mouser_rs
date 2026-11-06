//! `org.freedesktop.mouser1.Resolution` — one dpi setting.

use zbus::OwnedVariant;
use zbus::interface;

use crate::state::SharedDevice;

pub struct ResolutionIface {
    pub device: SharedDevice,
    pub profile: u32,
    pub index: u32,
}

#[interface(name = "org.freedesktop.mouser1.Resolution")]
impl ResolutionIface {
    /// `u`, read-only, constant.
    #[zbus(property)]
    async fn index(&self) -> u32 {
        self.index
    }

    /// `au`, read-only, constant. An array of the capability bits that are
    /// set — `1` separate x/y, `2` can be disabled — not a packed integer.
    /// Clients ignore values they do not know, so new bits are additive.
    #[zbus(property)]
    async fn capabilities(&self) -> Vec<u32> {
        todo!()
    }

    /// `b`, read-only. Change it with [`ResolutionIface::set_active`]: one
    /// resolution is always active, so it can only be set, never cleared.
    #[zbus(property, name = "IsActive")]
    async fn is_active(&self) -> bool {
        todo!()
    }

    /// `b`, read-only. Always false without the default-resolution
    /// capability. Change it with [`ResolutionIface::set_default`].
    #[zbus(property, name = "IsDefault")]
    async fn is_default(&self) -> bool {
        todo!()
    }

    /// `b`, read-write. Always false without the disable capability. Note a
    /// specific resolution may still refuse — e.g. the active one.
    #[zbus(property, name = "IsDisabled")]
    async fn is_disabled(&self) -> bool {
        todo!()
    }

    #[zbus(property, name = "IsDisabled")]
    async fn set_is_disabled(&mut self, value: bool) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `v`, read-write — a single `u` for both axes, or `(uu)` for x and y
    /// separately.
    ///
    /// Two rules the setter must enforce: the client may not change which of
    /// the two shapes this object exports, and the value must appear in
    /// [`ResolutionIface::resolutions`].
    #[zbus(property)]
    async fn resolution(&self) -> OwnedVariant {
        todo!("Variant::from(u32) or Variant::from((u32, u32)), then .into()")
    }

    #[zbus(property)]
    async fn set_resolution(&mut self, value: OwnedVariant) -> zbus::fdo::Result<()> {
        todo!("downcast to u32, else (u32, u32); reject a shape change")
    }

    /// `au`, read-only, constant, ascending. Empty when the device will not
    /// report its permitted values.
    #[zbus(property)]
    async fn resolutions(&self) -> Vec<u32> {
        todo!()
    }

    async fn set_active(&self) -> zbus::fdo::Result<()> {
        todo!()
    }

    async fn set_default(&self) -> zbus::fdo::Result<()> {
        todo!()
    }
}
