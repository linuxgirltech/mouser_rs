//! The D-Bus surface.
//!
//! One module per interface, each mirroring `docs/dbus-api.md`. The structs
//! and `#[interface]` attributes are in place — the property types and
//! read/write flags are the part that is painful to get wrong, so they are
//! filled in. Every body is yours.
//!
//! ## How a setting changes
//!
//! Three steps, and the daemon must not deviate:
//!
//! 1. the client writes properties, which stage changes in the [`Device`]
//!    model and set `Profile::is_dirty`;
//! 2. the client calls `Device.Commit()`, which **always succeeds** — the
//!    write to hardware happens asynchronously;
//! 3. if that write fails, the daemon emits `Device.Resync` and updates every
//!    property to the device's actual state.
//!
//! A commit can take several seconds on real hardware, so it must not be done
//! while holding a lock anything else needs, and `Commit()` must return
//! before it finishes.
//!
//! [`Device`]: libmouser::Device

pub mod button;
pub mod device;
pub mod led;
pub mod manager;
pub mod profile;
pub mod resolution;
