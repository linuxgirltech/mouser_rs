# mouser_rs

A Rust reimplementation of [libratbag](https://github.com/libratbag/libratbag)
and `ratbagd`: configuration of gaming mice and other configurable input
devices.

- **`libmouser`** — device model, `.device` database, hidraw access, drivers.
- **`mouserd`** — exposes it over D-Bus.

## Status

**Skeleton.** It compiles and is clippy-clean; it does nothing. Every
behavioural body is `todo!()`. No driver is implemented, so `mouserd` finds no
devices — and `run()` is itself `todo!()`, so it exits by panicking.

What is filled in, because getting it wrong costs more than writing it:

- D-Bus wire values and property types (`types.rs`, the `#[interface]` impls)
- struct shapes and trait signatures
- the kernel ABI reference for the hidraw ioctls (`hid.rs` module docs)
- the HID++ 2.0 framing rules (`driver/hidpp20.rs` module docs)
- packaging: D-Bus policy, systemd unit, dev udev rule
- `.device` entries for the hardware in `devices/`

Deliberately *not* filled in: all of it is yours.

## Layout

```
crates/libmouser/src/
  types.rs        wire values - D-Bus discriminants, filled in
  device.rs       Device/Profile/Resolution/Button/Led; set_* are the
                  validation seam
  db.rs           .device file parsing
  hid.rs          hidraw ioctls
  enumerate.rs    udev discovery + hotplug
  driver/mod.rs   Driver + DeviceBackend traits, registry
  driver/hidpp20.rs   Logitech HID++ 2.0
crates/mouserd/src/
  names.rs        bus name, paths - one file to flip for ratbagd compat
  state.rs        shared state
  dbus/           one module per interface
data/             dbus policy, systemd unit, dev udev rule
devices/          .device database entries
docs/             D-Bus API reference, driver notes
```

## Where to start

```bash
cargo build
cargo test        # no tests yet - that is also yours
```

Then `docs/driver-notes.md`. It covers what kind of driver this is (userspace
hidraw, not a kernel module), which of your devices to start with and why, how
to observe what the hardware is actually doing, and the HID++ 2.0 behaviours
that are not obvious from the wire format.

Suggested order of work:

1. `types.rs` conversions — pure functions, no hardware, immediate feedback.
2. `db.rs` — parse the files in `devices/`. Still no hardware.
3. `hid.rs` — the ioctls. First real contact; `HIDIOCGRAWINFO` on your G502
   returning `046d:c08b` is the milestone.
4. `driver/hidpp20.rs` — `Message` framing, then feature discovery. Log the
   whole feature table before modelling anything.
5. `device.rs` validation, then the `mouserd` D-Bus layer.

`ratbagctl` is already installed and is a working implementation of the same
thing — use it as an oracle. Don't run it and `mouserd` against one device at
the same time.

## Compatibility

`mouserd` owns `org.freedesktop.mouser1` and its own object paths, so it does
not collide with a running `ratbagd`. To make it a drop-in replacement
instead — so Piper and `ratbagctl` drive it unchanged — change the three
constants in `crates/mouserd/src/names.rs` and rename the files in
`data/dbus/`. The interfaces are otherwise the same shape.

## Licence

MIT, as libratbag is. The D-Bus API and the `.device` file format follow
libratbag's; no libratbag code is included.
