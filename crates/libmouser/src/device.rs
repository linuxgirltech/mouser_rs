//! The in-memory device model.
//!
//! Plain data. A driver fills it at probe time and writes it back on commit;
//! the D-Bus layer reads and mutates it in between. Nothing here touches
//! hardware.
//!
//! The `set_*` helpers are the validation seam: they are where "is this dpi
//! in the permitted list", "does this button accept macros" and so on belong,
//! so both the D-Bus layer and any in-process caller get the same checks.
//! They are also responsible for setting `Profile::is_dirty`.

use std::path::PathBuf;

use crate::error::Result;
use crate::types::{
    ButtonAction, Color, ColorDepth, DeviceType, Dpi, LedMode, ModelId, ResolutionCaps,
};

/// A configurable device and everything the API exposes about it.
#[derive(Debug, Clone)]
pub struct Device {
    pub model: ModelId,
    /// Presentable name, from the database rather than the device.
    pub name: String,
    pub device_type: DeviceType,
    /// Empty when the device reports none.
    pub firmware_version: String,
    /// Which driver is handling this, for logging and introspection.
    pub driver: String,
    /// The `/dev/hidraw*` this was probed from, if any.
    pub devnode: Option<PathBuf>,
    pub profiles: Vec<Profile>,
}

impl Device {
    pub fn new(model: ModelId, name: impl Into<String>, driver: impl Into<String>) -> Self {
        Self {
            model,
            name: name.into(),
            device_type: DeviceType::Mouse,
            firmware_version: String::new(),
            driver: driver.into(),
            devnode: None,
            profiles: Vec::new(),
        }
    }

    /// Bounds-check and return `Error::NoSuchIndex` with kind `"profile"`.
    pub fn profile(&self, index: u32) -> Result<&Profile> {
        todo!()
    }

    pub fn profile_mut(&mut self, index: u32) -> Result<&mut Profile> {
        todo!()
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        todo!()
    }

    /// Make `index` active and clear the flag elsewhere.
    ///
    /// Exactly one profile is active at all times, so this moves the flag
    /// rather than ever clearing the last one.
    pub fn set_active_profile(&mut self, index: u32) -> Result<()> {
        todo!()
    }

    /// True when any profile has unwritten changes.
    pub fn is_dirty(&self) -> bool {
        todo!()
    }

    /// Clear every dirty flag. Called after a successful commit.
    pub fn clear_dirty(&mut self) {
        todo!()
    }
}

/// One stored configuration on the device.
#[derive(Debug, Clone)]
pub struct Profile {
    pub index: u32,
    /// Empty when the device has no renameable profiles — the D-Bus layer
    /// treats an empty name as "cannot be changed".
    pub name: String,
    /// A disabled profile may hold junk bindings; rebind before committing.
    pub disabled: bool,
    pub is_active: bool,
    /// Set whenever anything in this profile changes; cleared on commit.
    pub is_dirty: bool,

    pub resolutions: Vec<Resolution>,
    pub buttons: Vec<Button>,
    pub leds: Vec<Led>,

    /// `1`, `0`, or `-1` when the device can neither read nor write it.
    pub angle_snapping: i32,
    /// Debounce in ms, or `-1` when not settable.
    pub debounce: i32,
    /// Permitted debounce times, ascending. Empty when unsupported.
    pub debounces: Vec<u32>,
    /// Report rate in Hz.
    pub report_rate: u32,
    /// Permitted report rates, ascending. Empty when unsupported.
    pub report_rates: Vec<u32>,
}

impl Profile {
    /// An empty profile. Drivers fill the rest in.
    pub fn new(index: u32) -> Self {
        Self {
            index,
            name: String::new(),
            disabled: false,
            is_active: false,
            is_dirty: false,
            resolutions: Vec::new(),
            buttons: Vec::new(),
            leds: Vec::new(),
            angle_snapping: -1,
            debounce: -1,
            debounces: Vec::new(),
            report_rate: 0,
            report_rates: Vec::new(),
        }
    }

    pub fn resolution(&self, index: u32) -> Result<&Resolution> {
        todo!()
    }

    pub fn resolution_mut(&mut self, index: u32) -> Result<&mut Resolution> {
        todo!()
    }

    pub fn button(&self, index: u32) -> Result<&Button> {
        todo!()
    }

    pub fn button_mut(&mut self, index: u32) -> Result<&mut Button> {
        todo!()
    }

    pub fn led(&self, index: u32) -> Result<&Led> {
        todo!()
    }

    pub fn led_mut(&mut self, index: u32) -> Result<&mut Led> {
        todo!()
    }

    /// Move the active flag to `index`. At least one resolution is always
    /// active, so this never clears the last one.
    pub fn set_active_resolution(&mut self, index: u32) -> Result<()> {
        todo!()
    }

    /// Move the default flag to `index`. Same invariant as above.
    pub fn set_default_resolution(&mut self, index: u32) -> Result<()> {
        todo!()
    }

    /// Check `rate` against `report_rates` before storing, and mark dirty.
    /// `Error::Unsupported` when the list is empty.
    pub fn set_report_rate(&mut self, rate: u32) -> Result<()> {
        todo!()
    }

    /// Check `ms` against `debounces` before storing, and mark dirty.
    pub fn set_debounce(&mut self, ms: i32) -> Result<()> {
        todo!()
    }
}

/// One dpi setting within a profile.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub index: u32,
    pub caps: ResolutionCaps,
    pub is_active: bool,
    pub is_default: bool,
    /// Always false when the device lacks `ResolutionCaps::disable`.
    pub is_disabled: bool,
    pub dpi: Dpi,
    /// Permitted dpi values, ascending. Empty when the device will not say.
    pub dpis: Vec<u32>,
}

impl Resolution {
    pub fn new(index: u32, dpi: Dpi, dpis: Vec<u32>) -> Self {
        Self {
            index,
            caps: ResolutionCaps::default(),
            is_active: false,
            is_default: false,
            is_disabled: false,
            dpi,
            dpis,
        }
    }

    /// Store a new dpi.
    ///
    /// Must reject: a change of variant shape (`Dpi::same_shape`), a
    /// `Separate` value when `caps.separate_xy` is unset, and any axis not in
    /// `dpis` when that list is non-empty.
    pub fn set_dpi(&mut self, dpi: Dpi) -> Result<()> {
        todo!()
    }
}

/// One physical button within a profile.
#[derive(Debug, Clone)]
pub struct Button {
    pub index: u32,
    pub action: ButtonAction,
    /// Action types this button accepts, as `Button.ActionTypes` wire values.
    pub action_types: Vec<u32>,
    /// Longest macro the device will store; 0 when macros are unsupported.
    pub macro_length: u32,
}

impl Button {
    pub fn new(index: u32, action_types: Vec<u32>) -> Self {
        Self {
            index,
            action: ButtonAction::None,
            action_types,
            macro_length: 0,
        }
    }

    /// Store a binding, rejecting types absent from `action_types` and macros
    /// longer than `macro_length`.
    pub fn set_action(&mut self, action: ButtonAction) -> Result<()> {
        todo!()
    }
}

/// One LED within a profile.
#[derive(Debug, Clone)]
pub struct Led {
    pub index: u32,
    pub mode: LedMode,
    /// Modes this LED supports, as `Led.Modes` wire values.
    pub modes: Vec<u32>,
    pub color: Color,
    pub color_depth: ColorDepth,
    /// Effect duration in ms, 0..=10000.
    pub effect_duration: u32,
    /// Brightness normalised to 0..=255. Devices with less range map it in an
    /// implementation-defined way.
    pub brightness: u32,
}

impl Led {
    pub const MAX_EFFECT_DURATION: u32 = 10_000;
    pub const MAX_BRIGHTNESS: u32 = 255;

    pub fn new(index: u32, modes: Vec<u32>, color_depth: ColorDepth) -> Self {
        Self {
            index,
            mode: LedMode::Off,
            modes,
            color: Color::default(),
            color_depth,
            effect_duration: 0,
            brightness: 0,
        }
    }

    /// Reject modes absent from `modes`.
    pub fn set_mode(&mut self, mode: LedMode) -> Result<()> {
        todo!()
    }

    /// Reject colours failing `Color::validate` for `color_depth`.
    pub fn set_color(&mut self, color: Color) -> Result<()> {
        todo!()
    }

    /// Reject anything above `MAX_EFFECT_DURATION`.
    pub fn set_effect_duration(&mut self, ms: u32) -> Result<()> {
        todo!()
    }

    /// Reject anything above `MAX_BRIGHTNESS`.
    pub fn set_brightness(&mut self, value: u32) -> Result<()> {
        todo!()
    }
}
