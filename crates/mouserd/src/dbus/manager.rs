//! `org.freedesktop.mouser1.Manager` — the entry point.

use zbus::OwnedObjectPath;
use zbus::interface;

use crate::names::API_VERSION;
use crate::state::SharedState;

pub struct Manager {
    pub state: SharedState,
}

impl Manager {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }
}

#[interface(name = "org.freedesktop.mouser1.Manager")]
impl Manager {
    /// `i`, read-only, constant.
    ///
    /// Clients must understand exactly this version. Goes away once the
    /// interface is declared stable.
    #[zbus(property)]
    async fn api_version(&self) -> i32 {
        API_VERSION
    }

    /// `ao`, read-only, mutable.
    ///
    /// Emit a `PropertiesChanged` for this whenever a device is added or
    /// removed — `devices_changed` is generated for you by the macro.
    #[zbus(property)]
    async fn devices(&self) -> Vec<OwnedObjectPath> {
        todo!()
    }
}
