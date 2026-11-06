# Writing the driver

Notes for implementing `crates/libmouser/src/driver/hidpp20.rs` and the
`hid.rs` layer under it. Nothing here is code you have to accept — it is the
map of the territory, the kernel-side mechanics, and how to see what your
device is actually doing.

## What kind of driver this is

This is a **userspace driver** over `hidraw`. The kernel's `hid-generic`
already owns your mouse and turns it into an evdev input device; you are not
replacing that. You open the raw HID character device alongside it and speak
the vendor's configuration protocol over feature/output reports.

That distinction matters if your goal is kernel work:

| | this exercise | kernel HID driver |
|---|---|---|
| Lives in | userspace, `/dev/hidraw*` | `drivers/hid/`, a module |
| Language | Rust | C (Rust-for-Linux does not cover HID yet) |
| Debug loop | run the binary | rebuild, reload module, maybe reboot |
| Gets wrong | an error value | an oops |

The protocol knowledge transfers completely; the kernel API does not. If you
want the kernel side afterwards, `drivers/hid/hid-logitech-hidpp.c` is the
same protocol as a real in-tree driver and the natural next read — the kernel
already implements HID++ for input-side features (battery, high-res scroll),
which is a useful cross-check when your userspace version disagrees with it.

## Your hardware, in a useful order

Everything you have is `Driver=hidpp20`, which is both lucky and the reason
the skeleton stubs that module specifically.

| Device | IDs | Why it is interesting |
|---|---|---|
| G502 HERO | `usb:046d:c08b` | **Start here.** Wired USB, `device_index` `0xff`, always present, no pairing. Onboard profiles and RGB. |
| MX Vertical | `usb:046d:c08a` wired, `usb:046d:407b` Unifying, `bluetooth:046d:b020` | Same device, three identities. Best illustration of why `Model` exists. |
| MX Master 2S / 3 | `usb:046d:4069` / `4082`, plus BT | Unifying wireless PIDs — exercises receiver device indices. |
| MX Master (orig) | six IDs across USB and BT | Several hardware revisions behind one name. |
| MX Master 3S | `bluetooth:046d:b034` only upstream | No receiver ID in the database. Expect to add one. |

The G502 is the only one that is always wired, so it is the only one where a
failed write cannot be blamed on the radio. Get it working first.

### Two hidraw nodes per device

Your G502 shows up as both `hidraw0` and `hidraw1` with identical
`HID_ID`. One node is the mouse/consumer collection, the other the vendor
collection that carries HID++. **Picking the right one is part of the job.**

The report descriptor tells you which: the HID++ node declares vendor-defined
usage page `0xFF00`–`0xFFFF` and reports `0x10` and `0x11` at the right
sizes. Read it from
`/sys/class/hidraw/hidrawN/device/report_descriptor` (binary) and parse, or
be pragmatic and probe both — write a root-feature query to each and keep the
one that answers. libratbag does roughly the latter.

### Unifying receivers and wireless PIDs

`usb:046d:4069` is not a USB device ID any `lsusb` will show you. When a
device is paired to a Unifying receiver, `hid-logitech-dj` enumerates the
receiver and creates a **child HID device per paired device**, with the
product ID set to the device's *wireless PID*. So the hidraw node you open is
the DJ child, not the receiver, and matching on `046d:4069` works.

Consequences for your driver:

- `device_index` is `0xff` only for wired. Through a receiver it is `0x01`–
  `0x06`, and a write to the wrong index silently configures a different mouse.
- A sleeping wireless device does not answer. Your `request()` needs a
  timeout, and "no reply" must be a distinguishable error from "error reply",
  because the correct response is to retry later rather than to give up.
- Bolt receivers are a newer protocol than Unifying. If the 3S does not match
  when dongled, read its actual `HID_ID` and extend the `.device` file.

## Seeing what the device does

Driver work is mostly observation. Four tools, cheapest first.

**1. What the kernel already knows.**

```bash
grep -r . /sys/class/hidraw/hidraw0/device/uevent
```

**2. The report descriptor** — the authoritative statement of what reports a
device accepts. `usbhid-dump` is installed:

```bash
sudo usbhid-dump -d 046d:c08b
```

Add `-e stream` to watch live reports instead of dumping descriptors.

**3. `hid-recorder`** is the better tool and is missing. It ships in
`hid-tools`:

```bash
sudo pacman -S --needed hid-tools
```

Then `sudo hid-recorder /dev/hidraw0` decodes the descriptor into readable
field names and timestamps every report, which beats reading hex.

**4. The reference implementation, as an oracle.** You have `ratbagctl` at
`/usr/local/bin/ratbagctl`. This is the single most valuable debugging asset
you have — a working implementation of the exact thing you are building:

```bash
sudo ratbagctl list
sudo ratbagctl "<device>" info
```

When your driver reports 6 buttons and ratbagd reports 11, ratbagd is right
and you have the wrong feature or the wrong offset. Compare against it
constantly. Do not run both daemons at once against the same device, though —
two writers on one hidraw node will interleave reports and confuse both.

Reading libratbag's `src/hidpp20.c` and `src/driver-hidpp20.c` is fair game
and much faster than rediscovering the protocol. The point of the exercise is
that you understand what you write, not that you never look at prior art.

## HID++ 2.0, the parts that bite

The framing is in the module docs. The things that are not obvious from the
wire format:

**Features are addressed by index, not ID, and indices differ per device.**
You cannot hardcode them. Query the root feature (always index `0x00`,
function `0x00`) with the feature ID in the params; it returns the index, or
`0` meaning unsupported. Cache the mapping at probe.

**Software ID must be non-zero.** The low nibble of byte 3 identifies your
traffic. Zero marks device-originated notifications — which arrive
unsolicited, interleaved with your replies, whenever the user moves the mouse.
Your read loop must skip anything that is not the reply you are waiting for,
with a bound so a device generating notifications faster than you drain them
cannot hang you.

**Error replies are HTTP-200-shaped.** A reply with `feature_index` `0x8f`
(HID++ 1.0) or `0xff` (2.0) is an error, not a result, and the read succeeds
normally. Check before you parse.

**Short vs long is not interchangeable.** Some functions demand a specific
report size and reject the other with an error. When in doubt, long.

**`0x8100 OnboardProfiles` is the keystone feature.** It decides whether the
device has real stored profiles or whether you synthesize a single one. Get
it before `AdjustableDpi` — the dpi you read means something different in
onboard mode versus host mode, and the G502 can be in either.

Suggested order: root → `0x0001 IFeatureSet` (enumerate what exists, log it
all) → `0x0003` firmware → `0x8100` profiles → `0x2201` dpi → `0x8060` rate →
`0x8070` LEDs. Log the whole feature table on your first run; it tells you
what the device can do before you commit to a model.

## Things worth getting right early

**Write access.** Your `/dev/hidraw0` is `crw------- root`. For development
use `data/udev/70-mouser-dev.rules`, which is scoped to the G502 and marked
dev-only — read the warning in it, because write access to a hidraw node is
the ability to send arbitrary HID reports to that device.

**Commit is asynchronous and slow.** The API promises `Commit()` always
succeeds and that failures arrive later as `Resync`. Writing profiles to a
G502 is many round trips. Do not hold a lock across it.

**A bricked profile is recoverable, a bricked device is not.** Writing
garbage to a profile sector is fine — Logitech G HUB or `ratbagctl` will
rewrite it. Writing to firmware-update or flash features is not. Stay out of
anything whose name mentions DFU.

**Test the parser without hardware.** `db.rs` and the HID++ `Message`
framing are pure functions over bytes and deserve unit tests; the ioctl layer
and the driver need a device. That split is why they are separate modules.

## Reading list

- `/usr/share/doc/libratbag/html/_sources/dbus.rst.txt` — the API you are implementing, installed locally
- <https://lekensteyn.nl/files/logitech/> — the HID++ reverse-engineering notes everyone works from
- `Documentation/hid/hidraw.rst` in the kernel tree — the ioctl contract
- `drivers/hid/hid-logitech-hidpp.c` — the same protocol, in-tree, in C
- libratbag's `src/driver-hidpp20.c` — the reference answer
