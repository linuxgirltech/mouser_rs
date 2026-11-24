//! Bus name, interface names and object paths, all in one place.
//!
//! mouserd is a **drop-in `ratbagd`**: it owns `org.freedesktop.ratbag1` and
//! exports the same interface names under the same object paths, so Piper and
//! `ratbagctl` drive it unchanged. Nothing has to be patched on their side —
//! which is the whole point, since it means a device mouserd supports and
//! libratbag does not becomes configurable in an already-installed GUI.
//!
//! # This now conflicts with ratbagd
//!
//! Two processes cannot own one well-known name, so mouserd and ratbagd are
//! mutually exclusive at runtime. A package therefore needs a `Conflicts=`
//! against libratbag, and the README's old claim that the two coexist is no
//! longer true.
//!
//! For development, build with `--features devel` to own
//! `org.freedesktop.ratbag_devel1` instead. Piper knows that name natively —
//! `piper/ratbagd.py` selects it when run in devel mode — so a devel build is
//! drivable by the same Piper while the system ratbagd keeps running. That is
//! the configuration to use while there is still a working `ratbagctl` on this
//! machine to compare against.
//!
//! # `API_VERSION` has to match exactly
//!
//! libratbag sets `ratbagd_api_version = 2` in its `meson.build`, and Piper
//! compares with `!=`:
//!
//! ```text
//! if self.api_version != api_version:
//!     raise RatbagdIncompatibleError(self.api_version or -1, api_version)
//! ```
//!
//! So neither older nor newer counts, and a mismatch surfaces to the user as
//! Piper refusing to start without naming a cause. Bump this only in step
//! with upstream.
//!
//! # Paths nest under the manager
//!
//! ratbagd's `RATBAGD_OBJ_ROOT` is `"/org/freedesktop/" RATBAG_DBUS_INTERFACE`
//! — so the root *is* the manager path, and every child hangs below it.
//! [`PATH_ROOT`] and [`MANAGER_PATH`] are therefore the same string, and the
//! `*_path` helpers below already mirror ratbagd's own layout
//! (`/profile/<device>/p<n>`, `/resolution/<device>/p<n>/r<n>`, …).

/// The well-known name mouserd owns on the system bus.
#[cfg(not(feature = "devel"))]
pub const BUS_NAME: &str = "org.freedesktop.ratbag1";
/// The well-known name mouserd owns on the system bus.
#[cfg(feature = "devel")]
pub const BUS_NAME: &str = "org.freedesktop.ratbag_devel1";

/// Prefix for every interface this daemon exports.
///
/// Kept in sync with the `#[interface]` attributes in [`crate::dbus`], which
/// need string literals and so cannot read this constant.
#[cfg(not(feature = "devel"))]
pub const IFACE_PREFIX: &str = "org.freedesktop.ratbag1";
/// Prefix for every interface this daemon exports.
#[cfg(feature = "devel")]
pub const IFACE_PREFIX: &str = "org.freedesktop.ratbag_devel1";

/// Root of the exported object tree — the same path as [`MANAGER_PATH`].
#[cfg(not(feature = "devel"))]
pub const PATH_ROOT: &str = "/org/freedesktop/ratbag1";
/// Root of the exported object tree — the same path as [`MANAGER_PATH`].
#[cfg(feature = "devel")]
pub const PATH_ROOT: &str = "/org/freedesktop/ratbag_devel1";

/// Where the manager object lives.
///
/// Piper derives this from the bus name rather than being told it
/// (`"/" + name.replace(".", "/")`), so it must stay in lockstep with
/// [`BUS_NAME`].
pub const MANAGER_PATH: &str = PATH_ROOT;

/// Reported by `Manager.APIVersion`.
///
/// Must equal libratbag's `ratbagd_api_version`. Clients match it exactly:
/// supporting an older or newer version is not enough.
pub const API_VERSION: i32 = 2;

/// `/org/freedesktop/ratbag1/device/<id>`
pub fn device_path(id: &str) -> String {
    format!("{PATH_ROOT}/device/{id}")
}

/// `/org/freedesktop/ratbag1/profile/<id>/p<profile>`
pub fn profile_path(id: &str, profile: u32) -> String {
    format!("{PATH_ROOT}/profile/{id}/p{profile}")
}

/// `/org/freedesktop/ratbag1/resolution/<id>/p<profile>/r<resolution>`
pub fn resolution_path(id: &str, profile: u32, resolution: u32) -> String {
    format!("{PATH_ROOT}/resolution/{id}/p{profile}/r{resolution}")
}

/// `/org/freedesktop/ratbag1/button/<id>/p<profile>/b<button>`
pub fn button_path(id: &str, profile: u32, button: u32) -> String {
    format!("{PATH_ROOT}/button/{id}/p{profile}/b{button}")
}

/// `/org/freedesktop/ratbag1/led/<id>/p<profile>/l<led>`
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
