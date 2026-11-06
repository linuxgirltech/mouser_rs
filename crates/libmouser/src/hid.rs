//! Thin `hidraw` wrapper — how drivers reach the hardware.
//!
//! Feature reports carry configuration on most devices; vendor protocols such
//! as HID++ use output/input reports instead.
//!
//! ## What you need to build here
//!
//! The ioctls live in `linux/hidraw.h`, type byte `'H'`:
//!
//! | ioctl | `_IOC` form | notes |
//! |---|---|---|
//! | `HIDIOCGRAWINFO` | `_IOR('H', 0x03, struct hidraw_devinfo)` | bus/vid/pid |
//! | `HIDIOCGRAWNAME(len)` | `_IOR('H', 0x04, len)` | 256-byte buffer is plenty |
//! | `HIDIOCGRAWPHYS(len)` | `_IOR('H', 0x05, len)` | physical path |
//! | `HIDIOCSFEATURE(len)` | `_IOWR('H', 0x06, len)` | send feature report |
//! | `HIDIOCGFEATURE(len)` | `_IOWR('H', 0x07, len)` | fetch feature report |
//!
//! The feature ioctls encode the buffer length in the request number, so
//! their opcodes are not compile-time constants and cannot go through
//! `rustix`'s const-generic `Getter`/`Setter`. Compute `_IOC` yourself and
//! call `libc::ioctl`, whose request argument is `c_ulong` on Linux.
//!
//! `_IOC` bit layout (`asm-generic/ioctl.h`): number 0..8, type 8..16,
//! size 16..30, direction 30..32 with read = 2, write = 1. For a 64-bit
//! kernel that gives `HIDIOCGRAWINFO == 0x8008_4803` and
//! `HIDIOCSFEATURE(8) == 0xC008_4806`, which are worth asserting in a test.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::BusType;

/// Mirrors `struct hidraw_devinfo`. The kernel declares the IDs signed; they
/// are not, so cast on the way out.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct HidrawDevinfo {
    pub bustype: u32,
    pub vendor: i16,
    pub product: i16,
}

/// What the kernel knows about a hidraw node.
#[derive(Debug, Clone)]
pub struct HidInfo {
    pub bus: BusType,
    pub vendor: u16,
    pub product: u16,
    /// The HID name string, often the marketing name.
    pub name: String,
    /// Physical path, e.g. `usb-0000:00:14.0-3/input0`.
    pub phys: String,
}

/// An open `/dev/hidraw*` node.
#[derive(Debug)]
pub struct HidrawDevice {
    file: File,
    path: PathBuf,
    info: HidInfo,
}

impl HidrawDevice {
    /// Open a node read-write and query its identity.
    ///
    /// Needs write access to the node — the daemon runs as root, or a udev
    /// rule grants it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        todo!("open read-write, then query_info")
    }

    /// `HIDIOCGRAWINFO` + `HIDIOCGRAWNAME` + `HIDIOCGRAWPHYS`.
    ///
    /// The name and phys strings come back NUL-terminated; truncate at the
    /// first NUL rather than trusting the returned length alone.
    fn query_info(file: &File) -> Result<HidInfo> {
        todo!()
    }

    pub fn info(&self) -> &HidInfo {
        &self.info
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Send a feature report. `data[0]` is the report ID — 0 when the device
    /// uses none.
    ///
    /// `HIDIOCSFEATURE` is declared read-write, so the buffer handed to the
    /// kernel must be writable even though nothing comes back through it.
    pub fn set_feature_report(&mut self, data: &[u8]) -> Result<usize> {
        todo!()
    }

    /// Fetch a feature report. Set `buf[0]` to the report ID first; the
    /// report is written back into `buf`.
    pub fn get_feature_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    /// Write an output report. `data[0]` is the report ID.
    pub fn write_output_report(&mut self, data: &[u8]) -> Result<usize> {
        todo!()
    }

    /// Read one input report. Blocks until the device sends something, so
    /// drivers that can stall want a timeout or non-blocking fd.
    pub fn read_input_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        todo!()
    }
}
