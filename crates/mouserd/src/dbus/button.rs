//! `org.freedesktop.mouser1.Button` — one button binding.

use zbus::OwnedVariant;
use zbus::interface;

use crate::state::SharedDevice;

pub struct ButtonIface {
    pub device: SharedDevice,
    pub profile: u32,
    pub index: u32,
}

#[interface(name = "org.freedesktop.mouser1.Button")]
impl ButtonIface {
    /// `u`, read-only, constant.
    #[zbus(property)]
    async fn index(&self) -> u32 {
        self.index
    }

    /// `(uv)`, read-write. First element is the action type, which must be
    /// one of [`ButtonIface::action_types`]. The variant depends on it:
    ///
    /// | type | name | variant |
    /// |---|---|---|
    /// | 0 | None | `u`, value 0 |
    /// | 1 | Button | `u`, the logical button number |
    /// | 2 | Special | `u`, a `0x40000000`-based special value |
    /// | 3 | Key | `u`, an evdev keycode |
    /// | 4 | Macro | `a(uu)`, each `(type, keycode)`; type 0 release, 1 press |
    /// | 1000 | Unknown | `u`, value 0 |
    #[zbus(property)]
    async fn mapping(&self) -> (u32, OwnedVariant) {
        todo!()
    }

    #[zbus(property)]
    async fn set_mapping(&mut self, value: (u32, OwnedVariant)) -> zbus::fdo::Result<()> {
        todo!("match on the action type, then downcast the variant to suit")
    }

    /// `au`, read-only, constant. Clients ignore types they do not know.
    #[zbus(property, name = "ActionTypes")]
    async fn action_types(&self) -> Vec<u32> {
        todo!()
    }
}
