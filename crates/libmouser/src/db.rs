//! The device database: `.device` files, same format libratbag uses.
//!
//! ```ini
//! [Device]
//! Name=ASUS ROG Chakram
//! DeviceMatch=usb:0b05:18e3;usb:0b05:18e5
//! DeviceType=mouse
//! Driver=asus
//!
//! [Driver/asus]
//! Profiles=3
//! Buttons=5
//! DpiRange=100:16000@100
//! Quirks=DOUBLE_DPI;STRIX_PROFILE
//! ```
//!
//! `[Device]` is fixed and required: `Name`, `DeviceMatch`, `DeviceType`,
//! `Driver`. `[Driver/<name>]` sections are driver-private — this layer keeps
//! them as strings and offers typed accessors, it never interprets them.
//! Sections nest (`[Driver/sinowealth/devices/V102]`) for drivers that pick a
//! sub-section after the device identifies itself.
//!
//! Keys seen across libratbag's 113 shipped files, for reference while
//! implementing drivers: `Profiles`, `Buttons`, `Leds`, `Dpis`, `DpiRange`,
//! `DpiList`, `ProfileType`, `MacroLength`, `Wireless`, `DeviceIndex`,
//! `DeviceName`, `DeviceVersion`, `SensorType`, `LedType`, `ButtonMapping`,
//! `Quirks` / `Quirk`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::{BusType, DeviceType};

/// Where shipped `.device` files live unless overridden.
pub const DEFAULT_DATA_DIR: &str = "/usr/share/libmouser";

/// One `DeviceMatch=` token, e.g. `usb:046d:c08b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceMatch {
    pub bus: BusType,
    pub vendor: u16,
    pub product: u16,
}

impl DeviceMatch {
    /// Parse `usb:046d:c08b`. IDs are lowercase hex, no `0x`.
    ///
    /// `None` for anything malformed, including trailing fields.
    pub fn parse(token: &str) -> Option<Self> {
        todo!()
    }
}

/// A span of dpi values the sensor accepts, from `DpiRange=min:max@step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiRange {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

impl DpiRange {
    /// Parse `100:16000@100`. Reject `step == 0` and `min > max`.
    pub fn parse(s: &str) -> Option<Self> {
        todo!()
    }

    /// Expand to every value in the range, for `Resolution.Resolutions`.
    pub fn values(&self) -> Vec<u32> {
        todo!()
    }
}

/// A parsed `.device` file.
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    /// Which file this came from, for diagnostics.
    pub path: PathBuf,
    pub name: String,
    pub device_type: DeviceType,
    pub driver: String,
    pub matches: Vec<DeviceMatch>,
    /// Section name without the `Driver/` prefix → key → value.
    ///
    /// `[Driver/asus]` lands under `"asus"`;
    /// `[Driver/sinowealth/devices/V102]` under `"sinowealth/devices/V102"`.
    pub driver_sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl DeviceEntry {
    /// The `[Driver/<driver>]` section for this entry's own driver.
    pub fn driver_section(&self) -> Option<&BTreeMap<String, String>> {
        todo!()
    }

    /// Look a key up in this entry's own driver section.
    pub fn get(&self, key: &str) -> Option<&str> {
        todo!()
    }

    /// Look a key up in a named section — for nested sub-sections.
    pub fn get_in(&self, section: &str, key: &str) -> Option<&str> {
        todo!()
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        todo!()
    }

    /// `1`/`true` and `0`/`false`.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        todo!()
    }

    /// Split a `;`-separated list value, dropping empties.
    pub fn get_list(&self, key: &str) -> Vec<&str> {
        todo!()
    }

    pub fn dpi_range(&self) -> Option<DpiRange> {
        todo!()
    }

    /// `DpiList=400;800;1600`, sorted ascending and deduped as the API
    /// requires.
    pub fn dpi_list(&self) -> Vec<u32> {
        todo!()
    }

    /// `Quirks=A;B`. Some files use the singular `Quirk=` — accept both.
    pub fn quirks(&self) -> Vec<&str> {
        todo!()
    }
}

/// Every known device.
#[derive(Debug, Default, Clone)]
pub struct DeviceDb {
    entries: Vec<DeviceEntry>,
}

impl DeviceDb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read every `*.device` file in `dir`.
    ///
    /// A file that fails to parse should be logged and skipped, not fatal —
    /// one bad file in a packaged database must not take the daemon down.
    pub fn load_dir(dir: impl AsRef<Path>) -> Result<Self> {
        todo!()
    }

    /// Parse one `.device` file.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<DeviceEntry> {
        todo!("read to string, hand to parse_entry")
    }

    pub fn entries(&self) -> &[DeviceEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(&mut self, entry: DeviceEntry) {
        self.entries.push(entry);
    }

    /// Find the entry covering this hardware.
    ///
    /// Bluetooth devices sometimes report their USB IDs, so try an exact
    /// bus/vid/pid match first and a bus-agnostic vid/pid match second.
    pub fn find(&self, bus: BusType, vendor: u16, product: u16) -> Option<&DeviceEntry> {
        todo!()
    }
}

/// Parse the contents of a `.device` file.
///
/// Comments are `#` or `;` at the start of a trimmed line. Blank lines are
/// skipped. Anything that is not a `[Section]` or `Key=Value` is an error, as
/// is a key before the first section header or an unknown section name.
fn parse_entry(path: &Path, text: &str) -> Result<DeviceEntry> {
    todo!()
}
