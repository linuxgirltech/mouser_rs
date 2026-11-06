//! Bus name, interface names and object paths, all in one place.
//!
//! To make mouserd a drop-in `ratbagd` — so Piper and `ratbagctl` talk to it
//! unchanged — change [`BUS_NAME`] and [`IFACE_PREFIX`] to
//! `org.freedesktop.ratbag1` and [`PATH_ROOT`] to `/org/freedesktop/ratbag`,
//! and rename the files in `data/dbus/`. Nothing else moves.

/// The well-known name mouserd owns on the system bus.
pub const BUS_NAME: &str = "org.freedesktop.mouser1";

/// Prefix for every interface this daemon exports.
pub const IFACE_PREFIX: &str = "org.freedesktop.mouser1";

/// Root of the exported object tree.
pub const PATH_ROOT: &str = "/org/freedesktop/mouser";

/// Where the manager object lives.
pub const MANAGER_PATH: &str = "/org/freedesktop/mouser1";

/// Reported by `Manager.APIVersion`.
///
/// A stopgap while the interface is in flux: clients must match it exactly,
/// supporting an older or newer version is not enough.
pub const API_VERSION: i32 = 1;

/// `/org/freedesktop/mouser/device/<id>`
pub fn device_path(id: &str) -> String {
    format!("{PATH_ROOT}/device/{id}")
}

/// `/org/freedesktop/mouser/profile/<id>/p<profile>`
pub fn profile_path(id: &str, profile: u32) -> String {
    format!("{PATH_ROOT}/profile/{id}/p{profile}")
}

/// `/org/freedesktop/mouser/resolution/<id>/p<profile>/r<resolution>`
pub fn resolution_path(id: &str, profile: u32, resolution: u32) -> String {
    format!("{PATH_ROOT}/resolution/{id}/p{profile}/r{resolution}")
}

/// `/org/freedesktop/mouser/button/<id>/p<profile>/b<button>`
pub fn button_path(id: &str, profile: u32, button: u32) -> String {
    format!("{PATH_ROOT}/button/{id}/p{profile}/b{button}")
}

/// `/org/freedesktop/mouser/led/<id>/p<profile>/l<led>`
pub fn led_path(id: &str, profile: u32, led: u32) -> String {
    format!("{PATH_ROOT}/led/{id}/p{profile}/l{led}")
}

/// Turn a device node into an object-path-safe id.
///
/// `/dev/hidraw3` becomes `hidraw3`. Object path elements accept only
/// `[A-Za-z0-9_]`, so everything else becomes `_`.
///
/// Clients are told not to rely on constructed paths, but keeping them stable
/// across restarts still makes debugging far easier.
pub fn device_id(devnode: Option<&std::path::Path>, fallback_index: usize) -> String {
    let raw = devnode
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("virtual{fallback_index}"));

    let sanitised: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    if sanitised.is_empty() {
        format!("virtual{fallback_index}")
    } else {
        sanitised
    }
}
