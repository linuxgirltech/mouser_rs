//! Value types shared by the device model, the drivers and the D-Bus layer.
//!
//! The numeric discriminants here are **wire values**: they cross D-Bus and
//! must match the published API exactly. They are filled in; the conversions
//! are not.
//!
//! Reference: `docs/dbus-api.md`.

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// Transport a device is attached over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    Usb,
    Bluetooth,
    Unknown,
}

impl BusType {
    /// The token used in `DeviceMatch=` lines and in `Device.Model` strings.
    pub fn from_token(token: &str) -> Self {
        match token {
            "usb" => BusType::Usb,
            "bluetooth" => BusType::Bluetooth,
            _ => BusType::Unknown,
        }
    }

    pub fn as_token(self) -> &'static str {
        match self {
            BusType::Usb => "usb",
            BusType::Bluetooth => "bluetooth",
            BusType::Unknown => "unknown",
        }
    }

    /// Map a `struct hidraw_devinfo.bustype` value.
    ///
    /// `linux/input.h`: `BUS_USB = 0x03`, `BUS_BLUETOOTH = 0x05`.
    pub fn from_hid_bus(bustype: u32) -> Self {
        match bustype {
            0x03 => BusType::Usb,
            0x05 => BusType::Bluetooth,
            _ => BusType::Unknown,
        }
    }
}

/// Stable identifier for a device *model*, exported as `Device.Model`.
///
/// Renders as `usb:046d:c08b:0` / `bluetooth:046d:b034:0`, or the literal
/// `unknown`. Vendor and product are 4-digit lowercase hex; vendor/product 0
/// are legal (test devices use them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelId {
    pub bus: BusType,
    pub vendor: u16,
    pub product: u16,
    /// Reserved for libmouser: bump when a vendor reuses IDs. Devices with
    /// the same IDs but different versions are different devices.
    pub version: u32,
}

impl ModelId {
    pub const fn new(bus: BusType, vendor: u16, product: u16) -> Self {
        Self {
            bus,
            vendor,
            product,
            version: 0,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            bus: BusType::Unknown,
            vendor: 0,
            product: 0,
            version: 0,
        }
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `unknown` when the bus is unknown, else
        // bus token, 4-digit lowercase hex vendor, 4-digit lowercase hex
        // product, decimal version — colon-separated.
        if self.bus == BusType::Unknown {
            return f.write_str("unknown");
        }
        write!(f, "{}:{:04x}:{:04x}:{}", self.bus.as_token(), self.vendor, self.product, self.version)
    }
}

/// `DeviceType=` in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceType {
    #[default]
    Mouse,
    Keyboard,
    Other,
}

impl FromStr for DeviceType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mouse" => Ok(DeviceType::Mouse),
            "keyboard" => Ok(DeviceType::Keyboard),
            "other" => Ok(DeviceType::Other), 
            other => Err(Error::invalid("DeviceType", format!("unknown `{other}`"))),
        }
    }
}

/// Bits of `Resolution.Capabilities`.
///
/// Exported as an `au` array of the set bits, not as a packed integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResolutionCaps {
    /// X and Y dpi can be set independently.
    pub separate_xy: bool,
    /// This resolution can be enabled and disabled. Note this is only the
    /// general capability — a specific resolution may still refuse, e.g. when
    /// it is the active one.
    pub disable: bool,
}

impl ResolutionCaps {
    pub const SEPARATE_XY: u32 = 1;
    pub const DISABLE: u32 = 2;

    /// Render as the `au` array `Resolution.Capabilities` exports.
    pub fn to_wire(self) -> Vec<u32> {
        let mut v = Vec::new();
        
        if self.separate_xy {
            v.push(Self::SEPARATE_XY);
        }

        if self.disable {
            v.push(Self::DISABLE);
        }

        v
    }
}

/// A resolution in dpi, either shared across both axes or per-axis.
///
/// This is the Rust side of the `Resolution.Resolution` variant: `Single`
/// exports as `u`, `Separate` as `(uu)`. A client must not switch a
/// resolution between the two shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dpi {
    Single(u32),
    Separate { x: u32, y: u32 },
}

impl Dpi {
    pub fn x(self) -> u32 {
        match self {
            Dpi::Single(x) => x,
            Dpi::Separate { x, .. } => x,
        }
    }

    pub fn y(self) -> u32 {
        match self {
            Dpi::Single(y) => y,
            Dpi::Separate { y, .. } => y, 
        }
    }

    /// True when both values describe the same variant shape. The D-Bus layer
    /// checks this before accepting a write.
    pub fn same_shape(self, other: Dpi) -> bool {
        matches!(
            (self, other),
            (Dpi::Single(_), Dpi::Single(_)) | (Dpi::Separate { .. }, Dpi::Separate{ .. })
        )
    }
}

/// `Button.ActionTypes` wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ActionType {
    None = 0,
    Button = 1,
    Special = 2,
    Key = 3,
    Macro = 4,
    Unknown = 1000,
}

impl ActionType {
    /// Anything unrecognised becomes `Unknown` — clients must ignore action
    /// types they do not know, and so must we.
    pub fn from_wire(value: u32) -> Self {
        match value {
            0 => ActionType::None,
            1 => ActionType::Button,
            2 => ActionType::Special,
            3 => ActionType::Key,
            4 => ActionType::Macro,
            _ => ActionType::Unknown,
        }
    }
}

/// Special button functions. All values are based at `1 << 30`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SpecialAction {
    Unknown = 0x4000_0000,
    DoubleClick = 0x4000_0001,
    WheelLeft = 0x4000_0002,
    WheelRight = 0x4000_0003,
    WheelUp = 0x4000_0004,
    WheelDown = 0x4000_0005,
    RatchetModeSwitch = 0x4000_0006,
    ResolutionCycleUp = 0x4000_0007,
    ResolutionCycleDown = 0x4000_0008,
    ResolutionUp = 0x4000_0009,
    ResolutionDown = 0x4000_000a,
    ResolutionAlternate = 0x4000_000b,
    ResolutionDefault = 0x4000_000c,
    ProfileCycleUp = 0x4000_000d,
    ProfileCycleDown = 0x4000_000e,
    ProfileUp = 0x4000_000f,
    ProfileDown = 0x4000_0010,
    SecondMode = 0x4000_0011,
    BatteryLevel = 0x4000_0012,
}

impl SpecialAction {
    pub const BASE: u32 = 0x4000_0000;

    /// `None` for a value this build does not know.
    pub fn from_wire(value: u32) -> Option<Self> {
        use SpecialAction::*;
        Some(match value {
            0x4000_0000 => Unknown,
            0x4000_0001 => DoubleClick,
            0x4000_0002 => WheelLeft,
            0x4000_0003 => WheelRight,
            0x4000_0004 => WheelUp,
            0x4000_0005 => WheelDown,
            0x4000_0006 => RatchetModeSwitch,
            0x4000_0007 => ResolutionCycleUp,
            0x4000_0008 => ResolutionCycleDown,
            0x4000_0009 => ResolutionUp,
            0x4000_000a => ResolutionDown,
            0x4000_000b => ResolutionAlternate,
            0x4000_000c => ResolutionDefault,
            0x4000_000d => ProfileCycleUp,
            0x4000_000e => ProfileCycleDown,
            0x4000_000f => ProfileUp,
            0x4000_0010 => ProfileDown,
            0x4000_0011 => SecondMode,
            0x4000_0012 => BatteryLevel,
            _ => return None,
        })
    }
}

/// One step of a macro: a press or release of an evdev keycode.
///
/// Exported inside `Button.Mapping` as `a(uu)`, each entry `(type, keycode)`
/// where type 0 is release and 1 is press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroEvent {
    pub pressed: bool,
    /// An evdev `KEY_*` code.
    pub keycode: u32,
}

impl MacroEvent {
    pub fn press(keycode: u32) -> Self {
        Self {
            pressed: true,
            keycode,
        }
    }

    pub fn release(keycode: u32) -> Self {
        Self {
            pressed: false,
            keycode,
        }
    }

    pub fn to_wire(self) -> (u32, u32) {
        (u32::from(self.pressed), self.keycode)
    }

    pub fn from_wire(kind: u32, keycode: u32) -> Self {
        Self {
            pressed: kind == 1,
            keycode,
        }
    }
}

/// The action bound to a button — the Rust side of `Button.Mapping` `(uv)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ButtonAction {
    /// Nothing bound. Variant is `u` of value 0.
    #[default]
    None,
    /// A logical button number. Variant is `u`.
    Button(u32),
    /// A special function. Variant is `u`.
    Special(SpecialAction),
    /// A single evdev keycode. Variant is `u`.
    Key(u32),
    /// A key sequence. Variant is `a(uu)`.
    Macro(Vec<MacroEvent>),
    /// The device reported something undecodable. Variant is `u` of value 0.
    Unknown,
}

impl ButtonAction {
    pub fn action_type(&self) -> ActionType {
        match self {
            ButtonAction::None => ActionType::None,
            ButtonAction::Button(_) => ActionType::Button,
            ButtonAction::Key(_) => ActionType::Key,
            ButtonAction::Macro(_) => ActionType::Macro,
            ButtonAction::Special(_) => ActionType::Special,
            ButtonAction::Unknown => ActionType::Unknown,
        }
    }
}

/// `Led.Modes` wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum LedMode {
    #[default]
    Off = 0,
    /// Constant brightness.
    On = 1,
    /// Cycles through a set of colours. Ignores `Led.Color`.
    Cycle = 2,
    Breathing = 3,
}

impl LedMode {
    pub fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            0 => LedMode::Off,
            1 => LedMode::On,
            2 => LedMode::Cycle,
            3 => LedMode::Breathing,
            _ => return None,
        })
    }
}

/// `Led.ColorDepth` wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum ColorDepth {
    /// 0 bits per colour — no colour control.
    Monochrome = 0,
    #[default]
    Rgb888 = 1,
    Rgb111 = 2,
}

impl ColorDepth {
    /// Largest value a single channel may take at this depth. Writes with
    /// bits set above this must be rejected.
    pub fn max_channel(self) -> u32 {
        match self {
            ColorDepth::Monochrome => 0,
            ColorDepth::Rgb888 => 255,
            ColorDepth::Rgb111 => 1
        }
    }

    pub fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            0 => ColorDepth::Monochrome,
            1 => ColorDepth::Rgb888,
            2 => ColorDepth::Rgb111, 
            _ => return None,
        })
    }
}

/// `Led.Color`, exported as the `(uuu)` RGB triplet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
}

impl Color {
    pub const fn new(red: u32, green: u32, blue: u32) -> Self {
        Self { red, green, blue }
    }

    /// Reject a colour with any channel above `depth.max_channel()`.
    pub fn validate(self, depth: ColorDepth) -> Result<()> {
        let max = depth.max_channel();

        for (name, value) in [("red", self.red), ("green", self.green), ("blue", self.blue),] {
            if value > max {
                return Err(Error::invalid("Color", format!("`{name}` channel `{value}` exceeds maximum `{max}`"),));
            }
        }
        Ok(())
    }

    pub fn to_wire(self) -> (u32, u32, u32) {
        (self.red, self.green, self.blue)
    }
}
