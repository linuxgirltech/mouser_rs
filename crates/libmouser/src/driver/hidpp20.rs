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

use crate::Error;
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

    /// Bytes before the parameters: report id, device index, feature index
    /// function-and-software-id.
    pub const HEADER_LEN: usize = 4;

    /// Build a short message, padding `params` to capacity and packing
    /// `function` with [`Message::SOFTWARE_ID`].
    pub fn short(device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Self> {
        Self::new(REPORT_ID_SHORT, device_index, feature_index, function, params)
    }

    pub fn long(device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Self> {
        Self::new(REPORT_ID_LONG, device_index, feature_index, function, params)
    }

    fn new(report_id: u8, device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Self> {
        let capacity = Self::param_len(report_id)?;
        if params.len() > capacity {
            return Err(Error::invalid("params", format!("{} bytes do not fit {capacity}", params.len())));
        }
        if function > 0x0f {
            return Err(Error::invalid("function", format!("{function:#04x} is not valid HID++")));
        }
        let mut padded = params.to_vec();
        padded.resize(capacity, 0);
        Ok(Self{
            report_id,
            device_index,
            feature_index,
            function: (function << 4) | Self::SOFTWARE_ID,
            params: padded,
        })
    }

    /// Param capacity for a report ID: total length minus the 4-byte header.
    pub fn param_len(report_id: u8) -> Result<usize> {
        match report_id {
            REPORT_ID_SHORT => Ok(SHORT_LEN - Self::HEADER_LEN),
            REPORT_ID_LONG => Ok(LONG_LEN - Self::HEADER_LEN),
            _ => Err(Error::invalid("report id", format!("{report_id:#04x} is not HID++"))),
        }
    }

    /// Serialise to the bytes written to the node.
    pub fn to_bytes(&self) -> Vec<u8> {
        debug_assert_eq!(
            Self::param_len(self.report_id).ok(), 
            Some(self.params.len()), 
            "params do not match the report id = a field was mutated directly"
        );
        let mut bytes = vec![
            self.report_id,
            self.device_index,
            self.feature_index,
            self.function,
        ];
        bytes.extend_from_slice(&self.params);
        bytes
    }

    /// Parse bytes read from the node. Reject unknown report IDs and buffers
    /// shorter than the report's full length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let &[report_id, device_index, feature_index, function, ref rest @ ..] = bytes else {
            return Err(Error::invalid("report", format!("{} bytes is shorter than a HID++ header", bytes.len())));
        };
        let capacity = Self::param_len(report_id)?;
        if rest.len() < capacity {
            return Err(Error::invalid("report", format!("a {report_id:#04x} report needs {capacity} params, got {}", rest.len())));
        }
        Ok(Self {
            report_id,
            device_index,
            feature_index,
            function,
            params: rest[..capacity].to_vec(),
        }) 
    }

    /// True when the device answered with an error instead of a result.
    pub fn is_error(&self) -> bool {
        matches!(self.feature_index, 0x8f | 0xff)
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
