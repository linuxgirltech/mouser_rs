//! Unit tests for `types`.
//!
//! Compiled as a child module of `crate::types` via `#[path]`.
//!
//! Every expected number here is a **literal from an external source** — the
//! published ratbagd D-Bus API for the wire values, `linux/input.h` for the
//! bus types. Asserting `from_wire(x as u32) == x` would only prove the two
//! hand-written lists agree with each other, not that either is correct.

use super::*;

// ---- BusType ----

/// `linux/input.h`: `BUS_USB = 0x03`, `BUS_BLUETOOTH = 0x05`.
#[test]
fn bus_type_maps_kernel_bus_values() {
    assert_eq!(BusType::from_hid_bus(0x03), BusType::Usb);
    assert_eq!(BusType::from_hid_bus(0x05), BusType::Bluetooth);

    // Anything else is a transport we do not configure over.
    assert_eq!(BusType::from_hid_bus(0x00), BusType::Unknown);
    assert_eq!(BusType::from_hid_bus(0x18), BusType::Unknown); // BUS_I2C
    assert_eq!(BusType::from_hid_bus(u32::MAX), BusType::Unknown);
}

/// These tokens appear in `DeviceMatch=` lines and in exported `Device.Model`
/// strings, so they are wire format and may not be renamed.
#[test]
fn bus_tokens_are_stable() {
    assert_eq!(BusType::Usb.as_token(), "usb");
    assert_eq!(BusType::Bluetooth.as_token(), "bluetooth");
    assert_eq!(BusType::Unknown.as_token(), "unknown");

    for bus in [BusType::Usb, BusType::Bluetooth, BusType::Unknown] {
        assert_eq!(BusType::from_token(bus.as_token()), bus);
    }

    // Unparseable tokens degrade rather than error — a database file naming a
    // bus we do not know should skip the entry, not fail the whole load.
    assert_eq!(BusType::from_token("USB"), BusType::Unknown);
    assert_eq!(BusType::from_token(""), BusType::Unknown);
    assert_eq!(BusType::from_token("i2c"), BusType::Unknown);
}

// ---- ModelId ----

#[test]
fn model_id_renders_the_published_format() {
    // Real IDs from devices/logitech-g502-hero.device and
    // devices/logitech-mx-master-3s.device.
    assert_eq!(
        ModelId::new(BusType::Usb, 0x046d, 0xc08b).to_string(),
        "usb:046d:c08b:0"
    );
    assert_eq!(
        ModelId::new(BusType::Bluetooth, 0x046d, 0xb034).to_string(),
        "bluetooth:046d:b034:0"
    );

    // Hex is lowercase and zero-padded to four digits; vendor/product 0 are
    // legal values, not a missing device.
    assert_eq!(
        ModelId::new(BusType::Usb, 0x0001, 0x000f).to_string(),
        "usb:0001:000f:0"
    );

    // An unknown bus collapses the whole string — there is nothing useful to
    // identify a model by.
    assert_eq!(ModelId::unknown().to_string(), "unknown");
    assert_eq!(
        ModelId::new(BusType::Unknown, 0x046d, 0xc08b).to_string(),
        "unknown"
    );
}

#[test]
fn model_id_version_is_rendered_decimal() {
    let mut id = ModelId::new(BusType::Usb, 0x046d, 0xc08b);
    id.version = 12;
    assert_eq!(id.to_string(), "usb:046d:c08b:12");
}

// ---- DeviceType ----

#[test]
fn device_type_parses_database_tokens() {
    assert_eq!("mouse".parse::<DeviceType>().unwrap(), DeviceType::Mouse);
    assert_eq!(
        "keyboard".parse::<DeviceType>().unwrap(),
        DeviceType::Keyboard
    );
    assert_eq!("other".parse::<DeviceType>().unwrap(), DeviceType::Other);

    assert!("Mouse".parse::<DeviceType>().is_err(), "tokens are lowercase");
    assert!("".parse::<DeviceType>().is_err());
    assert!("tablet".parse::<DeviceType>().is_err());

    assert_eq!(DeviceType::default(), DeviceType::Mouse);
}

// ---- ResolutionCaps ----

#[test]
fn resolution_caps_export_set_bits_only() {
    assert_eq!(ResolutionCaps::SEPARATE_XY, 1);
    assert_eq!(ResolutionCaps::DISABLE, 2);

    let none = ResolutionCaps::default();
    assert!(none.to_wire().is_empty(), "no capabilities means an empty array");

    let xy = ResolutionCaps {
        separate_xy: true,
        disable: false,
    };
    assert_eq!(xy.to_wire(), vec![1]);

    let disable = ResolutionCaps {
        separate_xy: false,
        disable: true,
    };
    assert_eq!(disable.to_wire(), vec![2]);

    let both = ResolutionCaps {
        separate_xy: true,
        disable: true,
    };
    assert_eq!(both.to_wire(), vec![1, 2], "ascending, no duplicates");
}

// ---- Dpi ----

#[test]
fn dpi_reads_both_axes_from_either_shape() {
    let single = Dpi::Single(1600);
    assert_eq!(single.x(), 1600);
    assert_eq!(single.y(), 1600);

    let separate = Dpi::Separate { x: 800, y: 1200 };
    assert_eq!(separate.x(), 800);
    assert_eq!(separate.y(), 1200);
}

/// The D-Bus layer rejects a write that would change a resolution's variant
/// shape, so this predicate gates a real error path.
#[test]
fn dpi_same_shape_compares_variants_not_values() {
    let a = Dpi::Single(400);
    let b = Dpi::Single(1600);
    let c = Dpi::Separate { x: 400, y: 400 };
    let d = Dpi::Separate { x: 800, y: 1200 };

    assert!(a.same_shape(b), "different values, same shape");
    assert!(c.same_shape(d));
    assert!(!a.same_shape(c), "equal dpi but a different variant");
    assert!(!c.same_shape(a));
}

// ---- ActionType ----

#[test]
fn action_type_wire_values() {
    assert_eq!(ActionType::None as u32, 0);
    assert_eq!(ActionType::Button as u32, 1);
    assert_eq!(ActionType::Special as u32, 2);
    assert_eq!(ActionType::Key as u32, 3);
    assert_eq!(ActionType::Macro as u32, 4);
    assert_eq!(ActionType::Unknown as u32, 1000);
}

#[test]
fn action_type_from_wire_degrades_to_unknown() {
    for want in [
        ActionType::None,
        ActionType::Button,
        ActionType::Special,
        ActionType::Key,
        ActionType::Macro,
    ] {
        assert_eq!(ActionType::from_wire(want as u32), want);
    }

    // Clients must ignore action types they do not know, and so must we.
    assert_eq!(ActionType::from_wire(5), ActionType::Unknown);
    assert_eq!(ActionType::from_wire(999), ActionType::Unknown);
    assert_eq!(ActionType::from_wire(1000), ActionType::Unknown);
    assert_eq!(ActionType::from_wire(u32::MAX), ActionType::Unknown);
}

// ---- SpecialAction ----

#[test]
fn special_action_endpoints_match_the_published_api() {
    assert_eq!(SpecialAction::BASE, 0x4000_0000);
    assert_eq!(SpecialAction::Unknown as u32, 0x4000_0000);
    assert_eq!(SpecialAction::DoubleClick as u32, 0x4000_0001);
    assert_eq!(SpecialAction::WheelLeft as u32, 0x4000_0002);
    assert_eq!(SpecialAction::ResolutionDown as u32, 0x4000_000a);
    assert_eq!(SpecialAction::SecondMode as u32, 0x4000_0011);
    assert_eq!(SpecialAction::BatteryLevel as u32, 0x4000_0012);
}

/// The discriminants and `from_wire`'s match arms are two separately
/// hand-written lists. This catches them drifting apart — it cannot catch
/// both being wrong in the same way, which is what the literals above are for.
#[test]
fn special_action_decodes_every_value_in_range() {
    for offset in 0..=0x12u32 {
        let wire = SpecialAction::BASE + offset;
        let decoded = SpecialAction::from_wire(wire)
            .unwrap_or_else(|| panic!("{wire:#010x} has a discriminant but no from_wire arm"));
        assert_eq!(
            decoded as u32, wire,
            "from_wire({wire:#010x}) decoded to a variant with a different discriminant"
        );
    }
}

#[test]
fn special_action_rejects_values_outside_the_range() {
    assert!(SpecialAction::from_wire(SpecialAction::BASE + 0x13).is_none());
    assert!(SpecialAction::from_wire(0).is_none(), "0 is not based at 1 << 30");
    assert!(SpecialAction::from_wire(1).is_none());
    assert!(SpecialAction::from_wire(u32::MAX).is_none());
}

// ---- MacroEvent ----

#[test]
fn macro_event_wire_encoding() {
    // `a(uu)` entries are (type, keycode), type 0 release and 1 press.
    assert_eq!(MacroEvent::press(30).to_wire(), (1, 30));
    assert_eq!(MacroEvent::release(30).to_wire(), (0, 30));

    assert!(MacroEvent::from_wire(1, 30).pressed);
    assert!(!MacroEvent::from_wire(0, 30).pressed);
    assert_eq!(MacroEvent::from_wire(1, 30).keycode, 30);

    // Only 1 means press; anything else is treated as a release rather than
    // rejected, so a device sending junk cannot break decoding.
    assert!(!MacroEvent::from_wire(2, 30).pressed);
    assert!(!MacroEvent::from_wire(u32::MAX, 30).pressed);
}

#[test]
fn macro_event_round_trips_through_the_wire_form() {
    for event in [MacroEvent::press(103), MacroEvent::release(103)] {
        let (kind, keycode) = event.to_wire();
        assert_eq!(MacroEvent::from_wire(kind, keycode), event);
    }
}

// ---- ButtonAction ----

#[test]
fn button_action_reports_its_action_type() {
    assert_eq!(ButtonAction::None.action_type(), ActionType::None);
    assert_eq!(ButtonAction::Button(3).action_type(), ActionType::Button);
    assert_eq!(ButtonAction::Key(30).action_type(), ActionType::Key);
    assert_eq!(
        ButtonAction::Special(SpecialAction::DoubleClick).action_type(),
        ActionType::Special
    );
    assert_eq!(
        ButtonAction::Macro(vec![MacroEvent::press(30)]).action_type(),
        ActionType::Macro
    );
    assert_eq!(ButtonAction::Unknown.action_type(), ActionType::Unknown);
}

#[test]
fn button_action_defaults_to_unbound() {
    assert_eq!(ButtonAction::default(), ButtonAction::None);
}

// ---- LedMode ----

#[test]
fn led_mode_wire_values() {
    assert_eq!(LedMode::Off as u32, 0);
    assert_eq!(LedMode::On as u32, 1);
    assert_eq!(LedMode::Cycle as u32, 2);
    assert_eq!(LedMode::Breathing as u32, 3);
    assert_eq!(LedMode::default(), LedMode::Off);
}

#[test]
fn led_mode_from_wire() {
    for want in [LedMode::Off, LedMode::On, LedMode::Cycle, LedMode::Breathing] {
        assert_eq!(LedMode::from_wire(want as u32), Some(want));
    }
    assert_eq!(LedMode::from_wire(4), None);
    assert_eq!(LedMode::from_wire(u32::MAX), None);
}

// ---- ColorDepth ----

#[test]
fn color_depth_wire_values() {
    assert_eq!(ColorDepth::Monochrome as u32, 0);
    assert_eq!(ColorDepth::Rgb888 as u32, 1);
    assert_eq!(ColorDepth::Rgb111 as u32, 2);
    assert_eq!(ColorDepth::default(), ColorDepth::Rgb888);
}

#[test]
fn color_depth_from_wire() {
    for want in [
        ColorDepth::Monochrome,
        ColorDepth::Rgb888,
        ColorDepth::Rgb111,
    ] {
        assert_eq!(ColorDepth::from_wire(want as u32), Some(want));
    }
    assert_eq!(ColorDepth::from_wire(3), None);
    assert_eq!(ColorDepth::from_wire(u32::MAX), None);
}

#[test]
fn color_depth_channel_ceilings() {
    assert_eq!(ColorDepth::Monochrome.max_channel(), 0);
    assert_eq!(ColorDepth::Rgb888.max_channel(), 255);
    assert_eq!(ColorDepth::Rgb111.max_channel(), 1);
}

// ---- Color ----

#[test]
fn color_accepts_values_at_the_ceiling() {
    assert!(Color::new(255, 255, 255).validate(ColorDepth::Rgb888).is_ok());
    assert!(Color::new(0, 0, 0).validate(ColorDepth::Rgb888).is_ok());
    assert!(Color::new(1, 0, 1).validate(ColorDepth::Rgb111).is_ok());

    // Monochrome has no colour control at all, so only all-zero is valid.
    assert!(Color::new(0, 0, 0).validate(ColorDepth::Monochrome).is_ok());
}

#[test]
fn color_rejects_each_channel_independently() {
    let depth = ColorDepth::Rgb111;
    for color in [
        Color::new(2, 0, 0),
        Color::new(0, 2, 0),
        Color::new(0, 0, 2),
    ] {
        assert!(
            color.validate(depth).is_err(),
            "{color:?} exceeds Rgb111 and must be rejected"
        );
    }

    assert!(Color::new(256, 0, 0).validate(ColorDepth::Rgb888).is_err());
    assert!(Color::new(1, 0, 0).validate(ColorDepth::Monochrome).is_err());
}

#[test]
fn color_validation_error_names_the_offending_channel() {
    let err = Color::new(0, 7, 0)
        .validate(ColorDepth::Rgb111)
        .expect_err("green exceeds Rgb111");

    let message = err.to_string();
    assert!(
        message.contains("green"),
        "the error should say which channel was wrong: {message}"
    );
    assert!(matches!(err, Error::InvalidValue { field: "Color", .. }));
}

#[test]
fn color_to_wire_is_rgb_order() {
    assert_eq!(Color::new(1, 2, 3).to_wire(), (1, 2, 3));
}
