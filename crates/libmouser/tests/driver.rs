//! The driver registry.
//!
//! Only the lookup half is covered here. `DriverRegistry::probe`,
//! `DeviceBackend`'s methods and all of `ManagedDevice` take a concrete
//! `&mut HidrawDevice`, whose only constructor is `HidrawDevice::open` on a
//! real node — so none of them can be exercised without hardware.
//! `tests/hardware.rs` covers what can be. Making the transport a trait that
//! `HidrawDevice` implements would close that gap; it is cheapest to do now,
//! before any driver is written against the concrete type.

use std::sync::Arc;

use libmouser::driver::ProbeContext;
use libmouser::{Driver, DriverRegistry, ProbeResult, Result};

struct FakeDriver(&'static str);

impl Driver for FakeDriver {
    fn id(&self) -> &'static str {
        self.0
    }

    fn probe(&self, _ctx: ProbeContext<'_>) -> Result<ProbeResult> {
        unreachable!("reaching a probe needs a real hidraw node")
    }
}

fn registry_with(ids: &[&'static str]) -> DriverRegistry {
    let mut registry = DriverRegistry::new();
    for id in ids {
        registry.register(Arc::new(FakeDriver(id)));
    }
    registry
}

#[test]
fn a_new_registry_knows_no_drivers() {
    let registry = DriverRegistry::new();
    assert_eq!(registry.ids().count(), 0);
    assert!(registry.get("hidpp20").is_none());
}

#[test]
fn the_builtin_registry_is_still_empty() {
    // Deliberate: a driver absent from `with_builtin` surfaces as
    // `Error::UnknownDriver` when a matching device turns up. Update this
    // number as each driver is finished and registered.
    assert_eq!(DriverRegistry::with_builtin().ids().count(), 0);
}

#[test]
fn a_registered_driver_is_retrievable_by_its_own_id() {
    let registry = registry_with(&["hidpp20"]);

    let driver = registry.get("hidpp20").expect("should be registered");
    assert_eq!(driver.id(), "hidpp20");
}

#[test]
fn registration_keys_on_the_drivers_own_id_not_a_caller_supplied_name() {
    // `register` takes only the driver, so the key can never disagree with
    // `Driver::id`.
    let registry = registry_with(&["asus"]);
    assert!(registry.get("asus").is_some());
    assert!(registry.get("hidpp20").is_none());
}

#[test]
fn an_unregistered_id_is_none_rather_than_a_panic() {
    let registry = registry_with(&["hidpp20"]);
    for id in ["", "HIDPP20", "hidpp10", "hidpp20 "] {
        assert!(registry.get(id).is_none(), "id {id:?} should not match");
    }
}

#[test]
fn ids_come_back_in_sorted_order() {
    let registry = registry_with(&["steelseries", "asus", "hidpp20", "etekcity"]);
    assert_eq!(
        registry.ids().collect::<Vec<_>>(),
        vec!["asus", "etekcity", "hidpp20", "steelseries"]
    );
}

#[test]
fn registering_the_same_id_twice_replaces_rather_than_duplicates() {
    let mut registry = registry_with(&["hidpp20"]);
    registry.register(Arc::new(FakeDriver("hidpp20")));

    assert_eq!(registry.ids().count(), 1);
}

#[test]
fn the_registry_is_cloneable_and_shares_its_drivers() {
    let registry = registry_with(&["hidpp20"]);
    let clone = registry.clone();

    assert_eq!(clone.ids().collect::<Vec<_>>(), vec!["hidpp20"]);

    let a = registry.get("hidpp20").unwrap();
    let b = clone.get("hidpp20").unwrap();
    assert!(Arc::ptr_eq(&a, &b), "a clone shares the Arc, not a new driver");
}

#[test]
fn get_hands_out_an_owned_handle_that_outlives_the_borrow() {
    // This is what lets `DriverRegistry::probe` call `driver.probe(...)` while
    // holding a `&mut HidrawDevice`: the borrow of the map ends at `get`.
    let driver = {
        let registry = registry_with(&["hidpp20"]);
        registry.get("hidpp20").unwrap()
    };
    assert_eq!(driver.id(), "hidpp20");
}

#[test]
fn the_debug_output_lists_the_registered_ids() {
    let registry = registry_with(&["asus", "hidpp20"]);
    let text = format!("{registry:?}");

    assert!(text.contains("asus"), "got {text}");
    assert!(text.contains("hidpp20"), "got {text}");
}

#[test]
fn a_default_registry_matches_a_new_one() {
    assert_eq!(DriverRegistry::default().ids().count(), 0);
}
