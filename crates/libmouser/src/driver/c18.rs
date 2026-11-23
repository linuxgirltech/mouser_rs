//! SOAI C18 — the vertical mouse libratbag never shipped a driver for.
//!
//! Sold as the Zelotes C-18, and as a Steren in Latin America. `12c9:2003`.
//!
//! ## Provenance
//!
//! The protocol was reverse engineered by Padre Adamo from USBPcap captures
//! of the vendor software, and implemented for libratbag in a pull request
//! that is still unmerged. Everything below is ported from there, not
//! rediscovered:
//!
//! * <https://github.com/libratbag/libratbag/pull/1877> — `driver-c18.c`,
//!   MIT licensed, © 2026 Padre Adamo. This is the source to port from.
//! * <https://github.com/PadreAdamo/c18_reverseengineer> — `PROTOCOL.md`.
//!   Documentation only: that repository carries no licence, so read it and
//!   port from the MIT pull request above.
//! * <https://github.com/libratbag/libratbag/issues/1526> — the support
//!   request, closed in 2023 for want of a volunteer.
//!
//! ## Wire format
//!
//! Two channels, on interface 2 (usage page `0xFF00`, no report IDs):
//!
//! * **8-byte control commands** — `[opcode, b1, b2, 0, 0, 0, 0, checksum]`,
//!   sent as a **Feature report**. The checksum makes all eight bytes sum to
//!   `0xFF`.
//! * **64-byte payloads** — LED, DPI and button tables, sent as an **Output
//!   report**.
//!
//! Those two paths need *opposite* handling of the leading byte, which is the
//! single most expensive thing to get wrong here. See [`C18::send_command`]
//! and [`C18::write_payload`].
//!
//! ## Transactions
//!
//! Every change is wrapped:
//!
//! ```text
//! 0x01 begin  →  the command or payload  →  0x02 select/commit
//! ```
//!
//! The trailing `0x02` is not a handshake — it selects the active profile and
//! doubles as the apply signal. Profile numbers on the wire are **1-based**,
//! so a `Profile` at index 0 is `0x01`.
//!
//! ## No readback
//!
//! Nothing can be queried. There is no "get" for DPI, buttons, LEDs or
//! anything else, so [`Driver::probe`] cannot discover the current state and
//! must assume the factory defaults. That makes libmouser the authority on
//! this device's configuration, which is exactly the case
//! [`DeviceBackend::reload`]'s no-op default is written for.

use std::time::Duration;

use crate::driver::{DeviceBackend, Driver, ProbeContext, ProbeResult};
use crate::error::{Result, Error};
use crate::hid::HidrawDevice;

// ------------------------------------------------------------ capabilities ---

pub const NUM_PROFILES: usize = 2;
pub const NUM_BUTTONS: usize = 11;
/// DPI stages, called resolutions in the D-Bus API.
pub const NUM_RESOLUTIONS: usize = 5;
pub const NUM_LEDS: usize = 1;

/// Longest macro the device will store, in key events.
pub const MAX_MACRO_KEYS: usize = 96;
/// Macro slots are numbered from here.
pub const MACRO_ID_BASE: u8 = 0x10;

/// Report rates in Hz, ascending, as `Profile::report_rates` wants them.
pub const REPORT_RATES: [u32; 4] = [125, 250, 500, 1000];

// -------------------------------------------------------------------- wire ---

/// An 8-byte control command, checksum included.
pub const COMMAND_LEN: usize = 8;
/// A 64-byte table payload.
pub const PAYLOAD_LEN: usize = 64;

/// Begin a transaction. Always `01 88 00 00 00 00 00 76`.
pub const OP_BEGIN: u8 = 0x01;
/// Select the active profile, and commit whatever preceded it.
pub const OP_SELECT_COMMIT: u8 = 0x02;
/// `03 <profile> <interval_ms>`.
pub const OP_SET_POLLING: u8 = 0x03;
/// `04 <profile> <stage 1..=5>`.
pub const OP_SET_DPI_STAGE: u8 = 0x04;
/// Prepare a DPI or LED table write. Shared by both.
pub const OP_PREPARE_TABLE: u8 = 0x0c;
/// Prepare a button table write.
pub const OP_PREPARE_BUTTON_TABLE: u8 = 0x0d;
/// Prepare a macro content upload. `b1` is the macro ID, not a profile.
pub const OP_PREPARE_EXT_DATA: u8 = 0x0f;

/// `b1` of [`OP_BEGIN`] — fixed, meaning unknown.
pub const BEGIN_ARG: u8 = 0x88;
/// `b2` of every `OP_PREPARE_*` command.
pub const PREPARE_ARG: u8 = 0x80;

/// Between consecutive packets. The device drops commands sent faster.
pub const PACKET_DELAY: Duration = Duration::from_millis(20);
/// After a macro payload, which takes longer to land than a table write.
pub const MACRO_SETTLE: Duration = Duration::from_millis(200);

// --------------------------------------------------------------------- dpi ---

pub const DPI_MIN: u32 = 200;
pub const DPI_MAX: u32 = 10_000;
/// The vendor software's slider granularity.
pub const DPI_STEP: u32 = 100;

/// Sensor calibration: dpi against the byte written to the DPI table.
///
/// Not a formula — the ratio drifts from 50 dpi per unit at the bottom to
/// about 32 at 6400 — so values between these points are linearly
/// interpolated and anything outside is clamped. Captured from the vendor
/// software; the only source of truth.
pub const DPI_CALIBRATION: [(u32, u8); 16] = [
    (200, 0x04),
    (300, 0x06),
    (400, 0x08),
    (600, 0x0d),
    (800, 0x12),
    (1200, 0x1b),
    (1600, 0x24),
    (2000, 0x2e),
    (2300, 0x34),
    (2400, 0x37),
    (2600, 0x3b),
    (3200, 0x49),
    (4800, 0x6f),
    (6400, 0xc9),
    (6600, 0xcc),
    (10000, 0xf3),
];

/// Where each DPI stage's x and y bytes sit in the 64-byte DPI table.
///
/// X and Y are written separately but the sensor cannot actually run them
/// apart — per-axis DPI is unsupported, tested on real hardware — so both
/// offsets always take the same value. [`crate::types::ResolutionCaps`] must
/// therefore leave `separate_xy` unset.
pub const DPI_STAGE_OFFSETS: [(usize, usize); NUM_RESOLUTIONS] =
    [(20, 28), (21, 29), (22, 30), (23, 31), (24, 32)];

// ----------------------------------------------------------------- helpers ---

/// The checksum byte closing an 8-byte command.
///
/// `0xFF - sum(bytes)`, so that all eight bytes of the finished command sum
/// to `0xFF` — which is the cheaper way to check one by eye in a hex dump.
fn checksum(bytes: &[u8; COMMAND_LEN - 1]) -> u8 {
    let sum = bytes.iter().copied().fold(0u8, u8::wrapping_add);
    0xFF - sum
}
/// The byte the DPI table wants for `dpi`.
///
/// Interpolate between the two bracketing [`DPI_CALIBRATION`] entries,
/// clamping to [`DPI_MIN`]/[`DPI_MAX`] outside the table.
fn dpi_to_byte(dpi: u32) -> u8 {
    let dpi = dpi.clamp(DPI_MIN, DPI_MAX);
    for pair in DPI_CALIBRATION.windows(2) {
        let (d0, b0) = pair[0];
        let (d1, b1) = pair[1];
        if (d0..=d1).contains(&dpi) {
            let (b0, b1) = (u32::from(b0), u32::from(b1));
            return (b0 + (dpi - d0) * (b1 - b0) / (d1 - d0)) as u8;
        }
    }
    DPI_CALIBRATION[DPI_CALIBRATION.len() - 1].1
}

/// The dpi a table byte stands for.
///
/// Only an estimate: the device has no readback, so this exists to turn the
/// defaults we wrote into something presentable. Nearest calibration point is
/// enough — do not round-trip it against [`dpi_to_byte`] and expect equality.
fn dpi_from_byte(byte: u8) -> u32 {
    DPI_CALIBRATION
        .iter()
        .min_by_key(|(_, b)| b.abs_diff(byte))
        .map(|(dpi, _)| *dpi)
        .expect("DPI_CALIBRATION is never empty")
}

/// The polling interval in milliseconds for a report rate in Hz.
///
/// `1000 / hz`, giving 1, 2, 4 or 8. [`Error::invalid`] for a rate outside
/// [`REPORT_RATES`] — the device ignores a bad interval silently, so it has
/// to be caught here.
///
/// [`Error::invalid`]: crate::Error::invalid
fn polling_interval(hz: u32) -> Result<u8> {
    let report_rates = self::REPORT_RATES;
    if !report_rates.contains(&hz) {
        return Err(Error::invalid("ReportRate", format!("{hz} Hz is not one of {report_rates:?}")));
    }
    Ok((1000 / hz) as u8)
}

/// A profile index as the wire numbers it: 1-based.
///
/// [`Error::NoSuchIndex`] beyond [`NUM_PROFILES`]; the device has no bounds
/// checking and writing to profile 3 corrupts something unidentified.
///
/// [`Error::NoSuchIndex`]: crate::Error::NoSuchIndex
fn wire_profile(index: u32) -> Result<u8> {
    let profiles = self::NUM_PROFILES as u32;
    if index >= profiles {
        return Err(Error::NoSuchIndex { kind: "profile", index });
    }
    Ok((index + 1) as u8)
}

// ----------------------------------------------------------------- command ---

/// One 8-byte control command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub opcode: u8,
    pub b1: u8,
    pub b2: u8,
}

impl Command {
    pub const fn new(opcode: u8, b1: u8, b2: u8) -> Self {
        Self { opcode, b1, b2 }
    }

    /// Begin a transaction.
    pub const fn begin() -> Self {
        Self::new(OP_BEGIN, BEGIN_ARG, 0x00)
    }

    /// Serialise, filling bytes 3..=6 with zero and appending [`checksum`].
    pub fn to_bytes(&self) -> [u8; COMMAND_LEN] {
        let head = [self.opcode, self.b1, self.b2, 0, 0, 0, 0];
        let mut bytes = [0u8; COMMAND_LEN];
        bytes[..COMMAND_LEN -1].copy_from_slice(&head);
        bytes[COMMAND_LEN -1] = checksum(&head);
        bytes
    }
}

// ------------------------------------------------------------ conversation ---

/// A configuration conversation with one C18.
pub struct C18<'a> {
    pub hidraw: &'a mut HidrawDevice,
}

impl<'a> C18<'a> {
    pub fn new(hidraw: &'a mut HidrawDevice) -> Self {
        Self { hidraw }
    }

    /// Does this node speak the C18 configuration protocol?
    ///
    /// All three of the device's nodes share its bus/vid/pid, so the database
    /// cannot tell them apart and this is the only discriminator. Upstream
    /// tests `usage_page(0) == 0xff00 && !has_report(1)`; writing to the wrong
    /// interface earns an `EPIPE`/stall.
    ///
    /// **Blocked:** this needs the report descriptor, and [`HidrawDevice`]
    /// cannot fetch one — `HIDIOCGRDESCSIZE` and `HIDIOCGRDESC` are not
    /// wrapped in `hid`. Either add them, or keep the sysfs path alongside the
    /// node and read `device/report_descriptor` the way `tests/hardware.rs`
    /// does.
    pub fn speaks_c18(hidraw: &HidrawDevice) -> Result<bool> {
        todo!()
    }

    /// Send an 8-byte control command as a Feature report, then wait
    /// [`PACKET_DELAY`].
    ///
    /// The Feature report is genuinely unnumbered, so **byte 0 is the opcode
    /// and must reach the device unchanged**. Prepending a `0x00`
    /// report-number byte — the usual hidraw convention — gets an immediate
    /// `EPIPE`/stall instead. [`HidrawDevice::set_feature_report`] is already
    /// right for this: it copies the slice verbatim and never rewrites byte 0.
    ///
    /// Contrast [`C18::write_payload`], which needs the opposite.
    pub fn send_command(&mut self, command: Command) -> Result<()> {
        let bytes = command.to_bytes();
        let written = self.hidraw.set_feature_report(&bytes)?;
        if written != COMMAND_LEN {
            return Err(Error::Commit(format!("opcode {:#04x}: write {written} of {COMMAND_LEN} bytes", command.opcode)));
        }
        std::thread::sleep(PACKET_DELAY);
        Ok(())
    }

    /// Write a 64-byte payload as an Output report, then wait
    /// [`PACKET_DELAY`].
    ///
    /// Send **65** bytes: a dummy `0x00`, then the payload. The kernel's
    /// hidraw `write()` path strips byte 0 as a report-number placeholder
    /// *only when it is literally `0x00`*, and passes anything else straight
    /// through. So a payload that genuinely begins with `0x00` — a macro
    /// header, a zero ack — silently loses its first byte unless the dummy is
    /// there to be eaten instead. Payloads starting with a non-zero byte work
    /// either way, which is what made this so slow to find upstream.
    pub fn write_payload(&mut self, payload: &[u8; PAYLOAD_LEN]) -> Result<()> {
        todo!()
    }

    /// `OP_BEGIN`, opening a transaction.
    pub fn begin(&mut self) -> Result<()> {
        self.send_command(Command::begin())
    }

    /// `OP_SELECT_COMMIT`, which both selects `index` and applies everything
    /// since [`C18::begin`].
    pub fn select_and_commit(&mut self, index: u32) -> Result<()> {
        let profile = wire_profile(index)?;
        self.send_command(Command::new(OP_SELECT_COMMIT, profile, 0x00))
    }

    /// Prepare, prime and write a 64-byte table.
    ///
    /// Three packets: the `prepare` command, a 64-byte primer (`ff ff 00..`),
    /// then the real payload. Shared by the DPI and LED tables, which is why
    /// they both use [`OP_PREPARE_TABLE`].
    pub fn write_table(
        &mut self,
        prepare: u8,
        index: u32,
        payload: &[u8; PAYLOAD_LEN],
    ) -> Result<()> {
        todo!()
    }
}

// ------------------------------------------------------------------ driver ---

/// Not registered in [`DriverRegistry::with_builtin`] until `probe` works.
///
/// [`DriverRegistry::with_builtin`]: crate::driver::DriverRegistry::with_builtin
pub struct C18Driver;

impl Driver for C18Driver {
    fn id(&self) -> &'static str {
        "c18"
    }

    /// Build the device from the known defaults.
    ///
    /// Unlike a HID++ probe there is nothing to interrogate — see the module
    /// docs on readback. So this reports [`C18::speaks_c18`] failures as
    /// `Error::ProbeFailed`, then synthesises two profiles of
    /// [`NUM_RESOLUTIONS`] resolutions, [`NUM_BUTTONS`] buttons and
    /// [`NUM_LEDS`] LED from the factory tables, and hands the backend the
    /// table bytes to mutate.
    ///
    /// Upstream retries the first exchange ten times at 300 ms. Worth keeping:
    /// the device is not ready the instant its node appears.
    fn probe(&self, ctx: ProbeContext<'_>) -> Result<ProbeResult> {
        todo!()
    }
}

/// Per-device state: the table bytes as last written.
///
/// These are the authority, because the device will not read them back. The
/// backend edits them from the [`Device`] and rewrites whole tables.
///
/// [`Device`]: crate::device::Device
pub struct C18Backend {
    pub dpi_table: [[u8; PAYLOAD_LEN]; NUM_PROFILES],
    pub button_table: [[u8; PAYLOAD_LEN]; NUM_PROFILES],
    pub next_macro_id: [u8; NUM_PROFILES],
}

impl DeviceBackend for C18Backend {
    /// Rewrite whichever tables changed, each in its own transaction.
    ///
    /// Only dirty profiles, and within them only dirty tables — a full
    /// rewrite is several seconds of 20 ms waits. This is what the per-item
    /// `is_dirty` flags on `Resolution`, `Button` and `Led` are for.
    fn commit(
        &mut self,
        device: &mut crate::device::Device,
        hidraw: &mut HidrawDevice,
    ) -> Result<()> {
        todo!()
    }

    /// Switch profile immediately rather than at commit.
    ///
    /// `OP_SELECT_COMMIT` does exactly this on its own, so there is no reason
    /// to make the caller wait for a commit.
    fn set_active_profile(
        &mut self,
        device: &mut crate::device::Device,
        hidraw: &mut HidrawDevice,
        index: u32,
    ) -> Result<()> {
        todo!()
    }

    // `reload` keeps the trait's no-op default: there is nothing to re-read.
}

/// Unit tests live in `tests/unit/`, compiled in as a child module so they
/// can reach the private helpers. Two levels up: this file is in
/// `src/driver/`.
#[cfg(test)]
#[path = "../../tests/unit/c18.rs"]
mod tests;
