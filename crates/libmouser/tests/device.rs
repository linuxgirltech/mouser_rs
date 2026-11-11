//! The in-memory device model and its validation seam.

use libmouser::{
    BusType, Button, ButtonAction, Color, ColorDepth, Device, DeviceType, Dpi, Error, Led, LedMode,
    MacroEvent, ModelId, Profile, Resolution, ResolutionCaps, SpecialAction,
};

fn model() -> ModelId {
    ModelId::new(BusType::Usb, 0x046d, 0xc08b)
}

/// A device with `count` empty profiles, the first one active.
fn device_with_profiles(count: u32) -> Device {
    let mut device = Device::new(model(), "Test Mouse", "hidpp20");
    device.profiles = (0..count).map(Profile::new).collect();
    if count > 0 {
        device.profiles[0].is_active = true;
    }
    device
}

// ----------------------------------------------------------------- Device ---

#[test]
fn a_new_device_starts_empty_with_mouse_as_the_type() {
    let device = Device::new(model(), "Test Mouse", "hidpp20");

    assert_eq!(device.model, model());
    assert_eq!(device.name, "Test Mouse");
    assert_eq!(device.driver, "hidpp20");
    assert_eq!(device.device_type, DeviceType::Mouse);
    assert_eq!(device.firmware_version, "");
    assert_eq!(device.devnode, None, "the caller sets this, not the driver");
    assert!(device.profiles.is_empty());
}

#[test]
fn a_new_device_is_not_dirty() {
    assert!(!device_with_profiles(3).is_dirty());
}

#[test]
fn an_out_of_range_profile_index_names_the_kind_and_the_index() {
    let device = device_with_profiles(2);

    let err = device.profile(7).unwrap_err();
    assert!(
        matches!(
            err,
            Error::NoSuchIndex {
                kind: "profile",
                index: 7
            }
        ),
        "got {err:?}"
    );
    assert_eq!(err.to_string(), "no such profile (index 7)");
}

#[test]
fn profile_lookup_is_zero_based_and_exclusive_at_the_end() {
    let device = device_with_profiles(2);
    assert!(device.profile(0).is_ok());
    assert!(device.profile(1).is_ok());
    assert!(device.profile(2).is_err());
}

#[test]
fn profile_mut_reports_the_same_error_as_profile() {
    let mut device = device_with_profiles(1);
    let err = device.profile_mut(5).unwrap_err();
    assert!(
        matches!(
            err,
            Error::NoSuchIndex {
                kind: "profile",
                index: 5
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn setting_the_active_profile_moves_the_flag_rather_than_adding_one() {
    let mut device = device_with_profiles(3);
    device.set_active_profile(2).unwrap();

    let active: Vec<u32> = device
        .profiles
        .iter()
        .filter(|p| p.is_active)
        .map(|p| p.index)
        .collect();
    assert_eq!(active, vec![2], "exactly one profile is active");
    assert_eq!(device.active_profile().unwrap().index, 2);
}

#[test]
fn setting_an_out_of_range_active_profile_changes_nothing() {
    let mut device = device_with_profiles(2);
    let err = device.set_active_profile(9).unwrap_err();
    assert!(matches!(err, Error::NoSuchIndex { kind: "profile", .. }), "got {err:?}");

    // The previous selection survives the failed call.
    assert_eq!(device.active_profile().unwrap().index, 0);
}

#[test]
fn a_device_with_no_profiles_has_no_active_profile() {
    let mut device = device_with_profiles(0);
    assert!(device.active_profile().is_none());
    assert!(device.set_active_profile(0).is_err());
}

#[test]
fn a_device_is_dirty_when_any_profile_is_and_clears_them_all() {
    let mut device = device_with_profiles(3);
    device.profiles[2].is_dirty = true;
    assert!(device.is_dirty());

    device.clear_dirty();
    assert!(!device.is_dirty());
    assert!(device.profiles.iter().all(|p| !p.is_dirty));
}

#[test]
fn setting_the_active_profile_does_not_by_itself_mark_the_device_dirty() {
    // `Device::set_active_profile` moves a flag the backend may apply
    // immediately; it is the backend's `set_active_profile` that decides
    // whether this needs a commit.
    let mut device = device_with_profiles(3);
    device.set_active_profile(1).unwrap();
    assert!(!device.is_dirty());
}

// ---------------------------------------------------------------- Profile ---

#[test]
fn a_new_profile_marks_unsupported_features_with_minus_one() {
    let profile = Profile::new(0);

    assert_eq!(profile.index, 0);
    assert_eq!(profile.name, "");
    assert_eq!(profile.angle_snapping, -1, "-1 means cannot read or write");
    assert_eq!(profile.debounce, -1);
    assert_eq!(profile.report_rate, 0);
    assert!(profile.debounces.is_empty());
    assert!(profile.report_rates.is_empty());
    assert!(!profile.disabled);
    assert!(!profile.is_active);
    assert!(!profile.is_dirty);
}

#[test]
fn each_child_lookup_reports_its_own_kind() {
    let profile = Profile::new(0);

    for (err, kind) in [
        (profile.resolution(0).unwrap_err(), "resolution"),
        (profile.button(0).unwrap_err(), "button"),
        (profile.led(0).unwrap_err(), "led"),
    ] {
        assert_eq!(err.to_string(), format!("no such {kind} (index 0)"));
    }
}

#[test]
fn each_mutable_child_lookup_reports_its_own_kind() {
    let mut profile = Profile::new(0);

    assert!(matches!(
        profile.resolution_mut(1).unwrap_err(),
        Error::NoSuchIndex { kind: "resolution", index: 1 }
    ));
    assert!(matches!(
        profile.button_mut(1).unwrap_err(),
        Error::NoSuchIndex { kind: "button", index: 1 }
    ));
    assert!(matches!(
        profile.led_mut(1).unwrap_err(),
        Error::NoSuchIndex { kind: "led", index: 1 }
    ));
}

#[test]
fn setting_the_active_resolution_moves_the_flag_and_marks_the_profile_dirty() {
    let mut profile = Profile::new(0);
    profile.resolutions = (0..3)
        .map(|i| Resolution::new(i, Dpi::Single(800), Vec::new()))
        .collect();
    profile.resolutions[0].is_active = true;

    profile.set_active_resolution(2).unwrap();

    let active: Vec<u32> = profile
        .resolutions
        .iter()
        .filter(|r| r.is_active)
        .map(|r| r.index)
        .collect();
    assert_eq!(active, vec![2]);
    assert!(profile.is_dirty);
}

#[test]
fn setting_the_default_resolution_moves_its_own_flag_independently() {
    let mut profile = Profile::new(0);
    profile.resolutions = (0..3)
        .map(|i| Resolution::new(i, Dpi::Single(800), Vec::new()))
        .collect();

    profile.set_active_resolution(0).unwrap();
    profile.set_default_resolution(2).unwrap();

    assert!(profile.resolutions[0].is_active);
    assert!(!profile.resolutions[0].is_default);
    assert!(profile.resolutions[2].is_default);
    assert!(!profile.resolutions[2].is_active);
}

#[test]
fn an_out_of_range_resolution_selection_leaves_the_profile_clean() {
    let mut profile = Profile::new(0);
    profile.resolutions = vec![Resolution::new(0, Dpi::Single(800), Vec::new())];

    assert!(profile.set_active_resolution(3).is_err());
    assert!(profile.set_default_resolution(3).is_err());
    assert!(!profile.is_dirty, "a rejected write must not mark dirty");
}

#[test]
fn report_rate_is_unsupported_when_the_device_lists_none() {
    let mut profile = Profile::new(0);
    let err = profile.set_report_rate(1000).unwrap_err();
    assert!(matches!(err, Error::Unsupported("Report Rate")), "got {err:?}");
    assert!(!profile.is_dirty);
}

#[test]
fn report_rate_must_be_one_of_the_listed_values() {
    let mut profile = Profile::new(0);
    profile.report_rates = vec![125, 250, 500, 1000];

    profile.set_report_rate(500).unwrap();
    assert_eq!(profile.report_rate, 500);
    assert!(profile.is_dirty);

    let err = profile.set_report_rate(750).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "ReportRate", .. }), "got {err:?}");
    assert_eq!(profile.report_rate, 500, "the rejected value is not stored");
}

#[test]
fn debounce_is_unsupported_when_the_device_lists_none() {
    let mut profile = Profile::new(0);
    let err = profile.set_debounce(4).unwrap_err();
    assert!(matches!(err, Error::Unsupported("Debounce")), "got {err:?}");
}

#[test]
fn debounce_must_be_one_of_the_listed_values() {
    let mut profile = Profile::new(0);
    profile.debounces = vec![2, 4, 8, 16];

    profile.set_debounce(8).unwrap();
    assert_eq!(profile.debounce, 8);
    assert!(profile.is_dirty);

    assert!(profile.set_debounce(5).is_err());
    assert_eq!(profile.debounce, 8);
}

#[test]
fn a_negative_debounce_is_rejected_even_though_it_is_the_unsupported_sentinel() {
    // `-1` means "not settable" when read back, but it is not a value a client
    // may write.
    let mut profile = Profile::new(0);
    profile.debounces = vec![2, 4, 8];

    let err = profile.set_debounce(-1).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Debounce", .. }), "got {err:?}");
}

// ------------------------------------------------------------- Resolution ---

#[test]
fn a_new_resolution_has_no_capabilities_and_no_flags() {
    let res = Resolution::new(1, Dpi::Single(800), vec![400, 800]);

    assert_eq!(res.index, 1);
    assert_eq!(res.dpi, Dpi::Single(800));
    assert_eq!(res.caps, ResolutionCaps::default());
    assert!(!res.is_active);
    assert!(!res.is_default);
    assert!(!res.is_disabled);
}

#[test]
fn a_resolution_cannot_change_between_single_and_separate() {
    let mut res = Resolution::new(0, Dpi::Single(800), Vec::new());
    res.caps.separate_xy = true;

    let err = res
        .set_dpi(Dpi::Separate { x: 800, y: 400 })
        .unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Resolution", .. }), "got {err:?}");
    assert_eq!(res.dpi, Dpi::Single(800), "the rejected value is not stored");
}

#[test]
fn a_separate_resolution_requires_the_separate_xy_capability() {
    let mut res = Resolution::new(0, Dpi::Separate { x: 800, y: 800 }, Vec::new());
    assert!(!res.caps.separate_xy);

    let err = res.set_dpi(Dpi::Separate { x: 1600, y: 400 }).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)), "got {err:?}");

    res.caps.separate_xy = true;
    res.set_dpi(Dpi::Separate { x: 1600, y: 400 }).unwrap();
    assert_eq!(res.dpi, Dpi::Separate { x: 1600, y: 400 });
}

#[test]
fn a_dpi_outside_the_permitted_list_is_rejected_on_either_axis() {
    let mut res = Resolution::new(0, Dpi::Separate { x: 400, y: 400 }, vec![400, 800, 1600]);
    res.caps.separate_xy = true;

    assert!(res.set_dpi(Dpi::Separate { x: 800, y: 1600 }).is_ok());

    for bad in [
        Dpi::Separate { x: 999, y: 800 },
        Dpi::Separate { x: 800, y: 999 },
    ] {
        let err = res.set_dpi(bad).unwrap_err();
        assert!(
            matches!(err, Error::InvalidValue { field: "Resolution", .. }),
            "{bad:?} should be rejected, got {err:?}"
        );
    }
}

#[test]
fn an_empty_permitted_list_accepts_any_dpi() {
    // An empty `dpis` means the device would not say, not that nothing is
    // allowed.
    let mut res = Resolution::new(0, Dpi::Single(800), Vec::new());
    res.set_dpi(Dpi::Single(31_337)).unwrap();
    assert_eq!(res.dpi, Dpi::Single(31_337));
}

#[test]
fn a_single_dpi_is_checked_against_the_list_on_both_axes() {
    let mut res = Resolution::new(0, Dpi::Single(400), vec![400, 800]);
    res.set_dpi(Dpi::Single(800)).unwrap();
    assert!(res.set_dpi(Dpi::Single(1600)).is_err());
}

// ----------------------------------------------------------------- Button ---

#[test]
fn a_new_button_is_unbound() {
    let button = Button::new(3, vec![0, 1, 3]);
    assert_eq!(button.index, 3);
    assert_eq!(button.action, ButtonAction::None);
    assert_eq!(button.macro_length, 0);
}

#[test]
fn a_button_accepts_only_the_action_types_it_advertises() {
    // Types 0 (None) and 1 (Button) only — no keys, no macros.
    let mut button = Button::new(0, vec![0, 1]);

    button.set_action(ButtonAction::Button(2)).unwrap();
    assert_eq!(button.action, ButtonAction::Button(2));

    let err = button.set_action(ButtonAction::Key(30)).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Mapping", .. }), "got {err:?}");
    assert_eq!(button.action, ButtonAction::Button(2), "unchanged after rejection");
}

#[test]
fn the_rejection_names_the_button_and_the_action_type() {
    let mut button = Button::new(7, vec![0]);
    let message = button.set_action(ButtonAction::Key(30)).unwrap_err().to_string();

    assert!(message.contains('7'), "should name the button, got {message:?}");
    assert!(message.contains('3'), "should name action type 3, got {message:?}");
}

#[test]
fn every_action_type_is_gated_by_the_advertised_list() {
    let all = vec![0, 1, 2, 3, 4, 1000];
    let mut button = Button::new(0, all);

    button.set_action(ButtonAction::None).unwrap();
    button.set_action(ButtonAction::Button(1)).unwrap();
    button
        .set_action(ButtonAction::Special(SpecialAction::WheelLeft))
        .unwrap();
    button.set_action(ButtonAction::Key(30)).unwrap();
    button
        .set_action(ButtonAction::Macro(vec![MacroEvent::press(30)]))
        .unwrap();
    button.set_action(ButtonAction::Unknown).unwrap();
}

#[test]
fn a_macro_longer_than_the_device_limit_is_rejected() {
    let mut button = Button::new(0, vec![4]);
    button.macro_length = 3;

    let events: Vec<MacroEvent> = (0..3).map(MacroEvent::press).collect();
    button.set_action(ButtonAction::Macro(events)).unwrap();

    let too_long: Vec<MacroEvent> = (0..4).map(MacroEvent::press).collect();
    let err = button
        .set_action(ButtonAction::Macro(too_long))
        .unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Mapping", .. }), "got {err:?}");
    assert!(err.to_string().contains('4'), "should name the length, got {err}");
}

#[test]
fn a_zero_macro_length_leaves_macro_length_unchecked() {
    // `macro_length` of 0 documents "macros unsupported", but the length check
    // is skipped at 0 — the gate is `action_types`, which here advertises 4.
    // Flagged deliberately: if a driver sets `action_types` to include macros
    // while leaving `macro_length` at 0, an unbounded macro is accepted.
    let mut button = Button::new(0, vec![4]);
    assert_eq!(button.macro_length, 0);

    let events: Vec<MacroEvent> = (0..500).map(MacroEvent::press).collect();
    assert!(button.set_action(ButtonAction::Macro(events)).is_ok());
}

// -------------------------------------------------------------------- Led ---

#[test]
fn a_new_led_is_off_and_black() {
    let led = Led::new(0, vec![0, 1], ColorDepth::Rgb888);
    assert_eq!(led.index, 0);
    assert_eq!(led.mode, LedMode::Off);
    assert_eq!(led.color, Color::default());
    assert_eq!(led.effect_duration, 0);
    assert_eq!(led.brightness, 0);
}

#[test]
fn led_limits_are_the_published_ones() {
    assert_eq!(Led::MAX_EFFECT_DURATION, 10_000);
    assert_eq!(Led::MAX_BRIGHTNESS, 255);
}

#[test]
fn an_led_accepts_only_the_modes_it_advertises() {
    // Off and Breathing, but not On or Cycle.
    let mut led = Led::new(0, vec![0, 3], ColorDepth::Rgb888);

    led.set_mode(LedMode::Breathing).unwrap();
    assert_eq!(led.mode, LedMode::Breathing);

    let err = led.set_mode(LedMode::Cycle).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Mode", .. }), "got {err:?}");
    assert_eq!(led.mode, LedMode::Breathing, "unchanged after rejection");
}

#[test]
fn an_led_colour_is_validated_against_its_own_depth() {
    let mut rgb = Led::new(0, vec![1], ColorDepth::Rgb888);
    rgb.set_color(Color::new(255, 128, 0)).unwrap();
    assert_eq!(rgb.color, Color::new(255, 128, 0));
    assert!(rgb.set_color(Color::new(256, 0, 0)).is_err());

    let mut dim = Led::new(1, vec![1], ColorDepth::Rgb111);
    dim.set_color(Color::new(1, 1, 0)).unwrap();
    assert!(dim.set_color(Color::new(2, 0, 0)).is_err());

    let mut mono = Led::new(2, vec![1], ColorDepth::Monochrome);
    mono.set_color(Color::new(0, 0, 0)).unwrap();
    assert!(mono.set_color(Color::new(1, 0, 0)).is_err());
}

#[test]
fn a_rejected_colour_is_not_stored() {
    let mut led = Led::new(0, vec![1], ColorDepth::Rgb111);
    led.set_color(Color::new(1, 1, 1)).unwrap();
    assert!(led.set_color(Color::new(5, 5, 5)).is_err());
    assert_eq!(led.color, Color::new(1, 1, 1));
}

#[test]
fn effect_duration_is_capped_at_ten_seconds() {
    let mut led = Led::new(0, vec![3], ColorDepth::Rgb888);

    led.set_effect_duration(0).unwrap();
    led.set_effect_duration(10_000).unwrap();
    assert_eq!(led.effect_duration, 10_000);

    let err = led.set_effect_duration(10_001).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "effect_duration", .. }), "got {err:?}");
    assert_eq!(led.effect_duration, 10_000);
}

#[test]
fn brightness_is_capped_at_a_byte() {
    let mut led = Led::new(0, vec![1], ColorDepth::Rgb888);

    led.set_brightness(0).unwrap();
    led.set_brightness(255).unwrap();
    assert_eq!(led.brightness, 255);

    let err = led.set_brightness(256).unwrap_err();
    assert!(matches!(err, Error::InvalidValue { field: "Brightness", .. }), "got {err:?}");
    assert_eq!(led.brightness, 255);
}

// ---------------------------------------------------- the dirty-flag seam ---

/// A profile holding one resolution, one button and one LED, all clean.
fn profile_with_children() -> Profile {
    let mut profile = Profile::new(0);
    profile.resolutions = vec![Resolution::new(0, Dpi::Single(800), Vec::new())];
    profile.buttons = vec![Button::new(0, vec![1])];
    profile.leds = vec![Led::new(0, vec![1], ColorDepth::Rgb888)];
    profile
}

/// Touch each child through its own setter.
fn dirty_every_child(profile: &mut Profile) {
    profile
        .resolution_mut(0)
        .unwrap()
        .set_dpi(Dpi::Single(1600))
        .unwrap();
    profile
        .button_mut(0)
        .unwrap()
        .set_action(ButtonAction::Button(2))
        .unwrap();
    profile
        .led_mut(0)
        .unwrap()
        .set_color(Color::new(1, 2, 3))
        .unwrap();
}

#[test]
fn each_child_records_its_own_change() {
    // Per-item flags are what let a backend write only what moved, which
    // matters when every write is a USB round trip.
    let mut profile = profile_with_children();
    dirty_every_child(&mut profile);

    assert!(profile.resolutions[0].is_dirty, "resolution");
    assert!(profile.buttons[0].is_dirty, "button");
    assert!(profile.leds[0].is_dirty, "led");
}

#[test]
fn a_change_to_any_child_makes_the_device_dirty() {
    // The flag is worthless if commit cannot see it: a daemon that guards on
    // `Device::is_dirty()` would discard the change without an error.
    for (what, touch) in [
        (
            "resolution",
            (|p: &mut Profile| {
                p.resolution_mut(0).unwrap().set_dpi(Dpi::Single(1600)).unwrap()
            }) as fn(&mut Profile),
        ),
        ("button", |p: &mut Profile| {
            p.button_mut(0)
                .unwrap()
                .set_action(ButtonAction::Button(2))
                .unwrap()
        }),
        ("led", |p: &mut Profile| {
            p.led_mut(0).unwrap().set_color(Color::new(1, 2, 3)).unwrap()
        }),
    ] {
        let mut device = device_with_profiles(1);
        *device.profile_mut(0).unwrap() = profile_with_children();
        touch(device.profile_mut(0).unwrap());

        assert!(
            device.is_dirty(),
            "a changed {what} must reach Device::is_dirty, or commit skips it"
        );
    }
}

#[test]
fn clearing_dirty_after_a_commit_reaches_every_child() {
    let mut device = device_with_profiles(1);
    *device.profile_mut(0).unwrap() = profile_with_children();
    dirty_every_child(device.profile_mut(0).unwrap());
    device.profile_mut(0).unwrap().is_dirty = true;

    device.clear_dirty();

    let profile = device.profile(0).unwrap();
    assert!(!profile.is_dirty, "the profile's own flag");
    assert!(!profile.resolutions[0].is_dirty, "resolution");
    assert!(!profile.buttons[0].is_dirty, "button");
    assert!(!profile.leds[0].is_dirty, "led");
    assert!(!device.is_dirty(), "nothing should remain dirty");
}
