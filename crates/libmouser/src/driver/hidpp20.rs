//! Logitech HID++ 2.0 — the driver most of the database needs.
//!
//! Protocol reference: <https://lekensteyn.nl/files/logitech/>
//!
//! ## Wire format
//!
//! Two packet sizes, distinguished by report ID. Every message is
//! `[report_id, device_index, feature_index, function_and_software_id,
//! ..params]`, padded to the full length.
//!
//! * short — report `0x10`, 7 bytes total, 3 param bytes
//! * long — report `0x11`, 20 bytes total, 16 param bytes
//!
//! The fourth byte packs the function in the high nibble and a software ID in
//! the low nibble. The software ID must be non-zero — zero marks device
//! notifications, which arrive unsolicited and must be skipped while waiting
//! for a reply. A reply whose `feature_index` is `0x8f` (HID++ 1.0) or `0xff`
//! (2.0) is an error, not a result.
//!
//! `device_index` is `0xff` for a directly-wired device; devices behind a
//! unifying receiver use `0x01`..`0x06`.
//!
//! ## Feature discovery
//!
//! Features are addressed by *index*, not ID, and indices differ per device.
//! The root feature is always index `0x00`: call function `0x00` with the
//! feature ID in the params to get its index. Index 0 back means the device
//! does not implement it.

use crate::driver::{Driver, ProbeContext, ProbeResult};
use crate::error::Result;
use crate::hid::HidrawDevice;

pub const REPORT_ID_SHORT: u8 = 0x10;
pub const REPORT_ID_LONG: u8 = 0x11;

pub const SHORT_LEN: usize = 7;
pub const LONG_LEN: usize = 20;

/// Directly wired over USB; receivers use 0x01–0x06.
pub const DEVICE_INDEX_WIRED: u8 = 0xff;

/// Feature IDs. These are the ones a mouse driver needs first; there are many
/// more in the protocol reference.
pub const FEATURE_ROOT: u16 = 0x0000;
/// `IFeatureSet` — how many features, and which.
pub const FEATURE_FEATURE_SET: u16 = 0x0001;
/// `DeviceInfo` — firmware version strings.
pub const FEATURE_DEVICE_FW_VERSION: u16 = 0x0003;
pub const FEATURE_ADJUSTABLE_DPI: u16 = 0x2201;
pub const FEATURE_REPORT_RATE: u16 = 0x8060;
pub const FEATURE_COLOR_LED_EFFECTS: u16 = 0x8070;
pub const FEATURE_ONBOARD_PROFILES: u16 = 0x8100;

/// One HID++ message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub report_id: u8,
    pub device_index: u8,
    /// Feature *index* on this device, not the feature ID.
    pub feature_index: u8,
    /// Function in the high nibble, software ID in the low nibble.
    pub function: u8,
    /// Padded to the report's param capacity.
    pub params: Vec<u8>,
}

impl Message {
    /// Identifies our traffic in replies. Must be non-zero.
    pub const SOFTWARE_ID: u8 = 0x0a;

    /// Build a short message, padding `params` to capacity and packing
    /// `function` with [`Message::SOFTWARE_ID`].
    pub fn short(device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Self> {
        todo!()
    }

    pub fn long(device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Self> {
        todo!()
    }

    /// Param capacity for a report ID: total length minus the 4-byte header.
    pub fn param_len(report_id: u8) -> Result<usize> {
        todo!()
    }

    /// Serialise to the bytes written to the node.
    pub fn to_bytes(&self) -> Vec<u8> {
        todo!()
    }

    /// Parse bytes read from the node. Reject unknown report IDs and buffers
    /// shorter than the report's full length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        todo!()
    }

    /// True when the device answered with an error instead of a result.
    pub fn is_error(&self) -> bool {
        todo!("feature_index 0x8f (HID++ 1.0) or 0xff (2.0)")
    }
}

/// A HID++ conversation with one device.
pub struct Hidpp20<'a> {
    pub hidraw: &'a mut HidrawDevice,
    pub device_index: u8,
}

impl<'a> Hidpp20<'a> {
    pub fn new(hidraw: &'a mut HidrawDevice, device_index: u8) -> Self {
        Self {
            hidraw,
            device_index,
        }
    }

    /// Send a request and read the matching reply.
    ///
    /// Skip reports that do not match on device index and function — those
    /// are notifications. Bound the number skipped so a chatty device cannot
    /// hang the caller.
    pub fn request(&mut self, message: &Message) -> Result<Message> {
        todo!()
    }

    /// Ask the root feature for the index a feature ID lives at.
    /// `None` when the device does not implement it.
    pub fn feature_index(&mut self, feature: u16) -> Result<Option<u8>> {
        todo!()
    }
}

/// Not registered in `DriverRegistry::with_builtin` until `probe` works.
pub struct Hidpp20Driver;

impl Driver for Hidpp20Driver {
    fn id(&self) -> &'static str {
        "hidpp20"
    }

    /// Discover features, then turn them into profiles, resolutions, buttons
    /// and LEDs. `FEATURE_ONBOARD_PROFILES` is the one that decides whether
    /// the device has real profiles or a single synthetic one.
    fn probe(&self, ctx: ProbeContext<'_>) -> Result<ProbeResult> {
        todo!()
    }
}
