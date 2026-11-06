//! `org.freedesktop.mouser1.Led` — one LED.

use zbus::interface;

use crate::state::SharedDevice;

pub struct LedIface {
    pub device: SharedDevice,
    pub profile: u32,
    pub index: u32,
}

#[interface(name = "org.freedesktop.mouser1.Led")]
impl LedIface {
    /// `u`, read-only, constant.
    #[zbus(property)]
    async fn index(&self) -> u32 {
        self.index
    }

    /// `u`, read-write. Must be one of [`LedIface::modes`].
    #[zbus(property)]
    async fn mode(&self) -> u32 {
        todo!()
    }

    #[zbus(property)]
    async fn set_mode(&mut self, value: u32) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `au`, read-only, constant. `0` off, `1` constant brightness, `2` cycle
    /// through colours (ignores `Color`), `3` breathing.
    #[zbus(property)]
    async fn modes(&self) -> Vec<u32> {
        todo!()
    }

    /// `(uuu)`, read-write. RGB, but only the least significant bits are
    /// valid — [`LedIface::color_depth`] says how many. Writes with any bit
    /// set above the depth must be rejected.
    #[zbus(property)]
    async fn color(&self) -> (u32, u32, u32) {
        todo!()
    }

    #[zbus(property)]
    async fn set_color(&mut self, value: (u32, u32, u32)) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `u`, read-only, constant. `0` monochrome, `1` eight bits per colour,
    /// `2` one bit per colour. Clients must not touch the colour of an LED
    /// whose depth they do not recognise.
    #[zbus(property, name = "ColorDepth")]
    async fn color_depth(&self) -> u32 {
        todo!()
    }

    /// `u`, read-write. Milliseconds, 0..=10000.
    #[zbus(property, name = "EffectDuration")]
    async fn effect_duration(&self) -> u32 {
        todo!()
    }

    #[zbus(property, name = "EffectDuration")]
    async fn set_effect_duration(&mut self, value: u32) -> zbus::fdo::Result<()> {
        todo!()
    }

    /// `u`, read-write. Normalised 0..=255. An LED with less range maps the
    /// value in a driver-defined way.
    #[zbus(property)]
    async fn brightness(&self) -> u32 {
        todo!()
    }

    #[zbus(property)]
    async fn set_brightness(&mut self, value: u32) -> zbus::fdo::Result<()> {
        todo!()
    }
}
