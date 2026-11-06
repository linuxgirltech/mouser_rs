# D-Bus API

Compact reference for what `mouserd` exports. The shape follows ratbagd's
published API; the full upstream prose is installed locally at
`/usr/share/doc/libratbag/html/_sources/dbus.rst.txt` and is worth reading
once in full before implementing the daemon.

Bus name, interface prefix and path root are all in
`crates/mouserd/src/names.rs`.

**Flags:** *constant* never changes for the object's lifetime. *mutable* may
change and emits `org.freedesktop.DBus.Properties.PropertiesChanged`.

## Changing a setting

Three steps, and the ordering is load-bearing:

1. write properties — this only stages changes;
2. call `Device.Commit()` — **always succeeds**, writes asynchronously;
3. on write failure the daemon emits `Device.Resync` and updates every
   property to the device's real state.

A commit can take several seconds on real hardware. Clients are expected to
tolerate a `Resync` arriving long after `Commit()` returned.

## Manager

Path: `/org/freedesktop/mouser1`

| Member | Type | Flags | Notes |
|---|---|---|---|
| `APIVersion` | `i` | ro, constant | Clients must match **exactly**; neither older nor newer counts. Goes away when the API stabilises. |
| `Devices` | `ao` | ro, mutable | The `Device` objects. |

## Device

| Member | Type | Flags | Notes |
|---|---|---|---|
| `Model` | `s` | ro, constant | `usb:046d:c08b:0`, `bluetooth:…:0`, or `unknown`. 4-digit lowercase hex IDs, decimal version. Vendor/product `0` are valid. Clients ignore unknown prefixes. |
| `Name` | `s` | ro, constant | Presentable. |
| `FirmwareVersion` | `s` | ro, constant | Empty when unreported. |
| `Profiles` | `ao` | ro, mutable* | *Mutable only where profiles can be added/removed. |
| `Commit()` | → `()` | | Always succeeds. |
| `Resync` | signal | | Resync your property values. |

## Profile

| Member | Type | Flags | Notes |
|---|---|---|---|
| `Index` | `u` | ro, constant | Zero-based. |
| `Name` | `s` | rw, mutable | Empty ⇒ cannot be renamed. |
| `Disabled` | `b` | rw, mutable | A disabled profile may hold junk bindings — rebind before committing. |
| `IsActive` | `b` | ro, mutable | Set via `SetActive()`; one profile is always active, so it cannot be cleared. |
| `IsDirty` | `b` | ro, mutable | True once any other property is written. |
| `Resolutions` | `ao` | ro, mutable* | |
| `Buttons` | `ao` | ro, constant | |
| `Leds` | `ao` | ro, constant | |
| `AngleSnapping` | `i` | rw, mutable | `1`/`0`, or `-1` when unsupported. |
| `Debounce` | `i` | rw, mutable | ms; must be in `Debounces`. `-1` when not settable. |
| `Debounces` | `au` | constant | Ascending; empty when unsupported. |
| `ReportRate` | `u` | rw, mutable | Hz; must be in `ReportRates`. |
| `ReportRates` | `au` | constant | Ascending; empty when unsupported. |
| `SetActive()` | → `()` | | |

> The upstream doc marks `Debounces` and `ReportRates` "read-write,
> constant", which is self-contradictory for lists of permitted values. This
> skeleton exports them read-only.

## Resolution

| Member | Type | Flags | Notes |
|---|---|---|---|
| `Index` | `u` | ro, constant | |
| `Capabilities` | `au` | ro, constant | Array of set bits, not a packed int: `1` separate x/y, `2` can be disabled. Ignore unknown values. |
| `IsActive` | `b` | ro, mutable | Via `SetActive()`. |
| `IsDefault` | `b` | ro, mutable | Via `SetDefault()`; always false without the capability. |
| `IsDisabled` | `b` | rw, mutable | Always false without the capability; a specific resolution may still refuse (e.g. the active one). |
| `Resolution` | `v` | rw, mutable | `u` for both axes, or `(uu)` for x/y. **A client must not change which shape an object exports.** Value must be in `Resolutions`. |
| `Resolutions` | `au` | ro, constant | Ascending; empty when unreported. |
| `SetActive()`, `SetDefault()` | → `()` | | |

## Button

| Member | Type | Flags |
|---|---|---|
| `Index` | `u` | ro, constant |
| `Mapping` | `(uv)` | rw, mutable |
| `ActionTypes` | `au` | ro, constant |

`Mapping`'s first element is the action type; the variant depends on it:

| Type | Name | Variant |
|---|---|---|
| 0 | None | `u`, value 0 |
| 1 | Button | `u`, logical button number |
| 2 | Special | `u`, a special value (below) |
| 3 | Key | `u`, evdev keycode |
| 4 | Macro | `a(uu)`, each `(type, keycode)`; type 0 release, 1 press |
| 1000 | Unknown | `u`, value 0 |

## Led

| Member | Type | Flags | Notes |
|---|---|---|---|
| `Index` | `u` | ro, constant | |
| `Mode` | `u` | rw, mutable | Must be in `Modes`. |
| `Modes` | `au` | ro, constant | `0` off, `1` constant, `2` colour cycle (ignores `Color`), `3` breathing. |
| `Color` | `(uuu)` | rw, mutable | RGB; only the low bits per `ColorDepth` are valid, and writes must have all higher bits zero. |
| `ColorDepth` | `u` | ro, constant | `0` monochrome, `1` 8 bits/colour, `2` 1 bit/colour. Do not touch colour at an unknown depth. |
| `EffectDuration` | `u` | rw, mutable | ms, 0–10000. |
| `Brightness` | `u` | rw, mutable | Normalised 0–255. |

## Special button functions

All based at `0x40000000` (`1 << 30`):

| Offset | Function | Offset | Function |
|---|---|---|---|
| 0 | Unknown | 10 | Resolution down |
| 1 | Doubleclick | 11 | Resolution alternate |
| 2 | Wheel left | 12 | Resolution default |
| 3 | Wheel right | 13 | Profile cycle up |
| 4 | Wheel up | 14 | Profile cycle down |
| 5 | Wheel down | 15 | Profile up |
| 6 | Ratchet mode switch | 16 | Profile down |
| 7 | Resolution cycle up | 17 | Second mode |
| 8 | Resolution cycle down | 18 | Battery level |
| 9 | Resolution up | | |

## Object paths

Constructed from the device for debuggability, e.g.
`/org/freedesktop/mouser/button/hidraw0/p0/b10`. **Clients must not rely on
constructing these** — they walk the `ao` properties instead.
