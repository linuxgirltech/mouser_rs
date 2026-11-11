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

use std::ffi::CStr;
use std::fs::{self, File};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::path::{Path, PathBuf};

use crate::Error;
use crate::error::Result;
use crate::types::BusType;

/// Direction bit for "userspace writes", i.e. the kernel reads your buffer.
const IOC_WRITE: u32 = 1;
/// Direction bit for "userspace reads", i.e. the kernel writes your buffer.
const IOC_READ: u32 = 2;

const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + 8; // _IOC_NRBITS   = 8
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + 8; // _IOC_TYPEBITS = 8
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + 14; // _IOC_SIZEBITS = 14 on x86

/// Largest payload the 14-bit size field can encode. A longer buffer would
/// overflow into the direction bits and silently name a different ioctl.
const IOC_SIZE_MAX: u32 = (1 << 14) - 1;

/// The hidraw ioctl type byte.
const HID: u32 = b'H' as u32;

/// Build an ioctl request number — the Rust spelling of `_IOC`.
const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> u32 {
    (dir << IOC_DIRSHIFT) | (ty << IOC_TYPESHIFT) | (nr << IOC_NRSHIFT) | (size << IOC_SIZESHIFT)
}

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
        let path = path.as_ref();
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|err| Error::io(path, err))?;
        let info = Self::query_info(&file)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            info,
        })
    }

    /// `HIDIOCGRAWNAME` / `HIDIOCGRAWPHYS` — identical shape, different `nr`.
    fn read_string_ioctl(fd: BorrowedFd<'_>, nr: u32, field: &'static str) -> Result<String> {
        let mut buf = [0u8; 256];
        let req = ioc(IOC_READ, HID, nr, buf.len() as u32);

        unsafe { Self::ioctl(fd, req, buf.as_mut_ptr())? };

        let str = CStr::from_bytes_until_nul(&buf).map_err(|_| Error::invalid(field, "no NUL terminator"))?;

        Ok(str.to_string_lossy().into_owned())
    }

    /// Issue an ioctl on `fd`.
    ///
    /// # Safety
    ///
    /// `request` must encode the size of `*arg` exactly: the kernel reads and
    /// writes `_IOC_SIZE(request)` bytes through the pointer, not `size_of::<T>()`.
    ///
    /// `arg` must be valid for reads if `request`'s direction includes `IOC_WRITE`,
    /// and valid for writes if it includes `IOC_READ` — the direction is from
    /// userspace's point of view, so `IOC_READ` is the case where the kernel
    /// writes into your allocation.
    unsafe fn ioctl<T>(fd: BorrowedFd<'_>, request: u32, arg: *mut T) -> Result<i32> {
        let ret = unsafe { libc::ioctl(fd.as_raw_fd(), request as libc::Ioctl, arg) };
        if ret < 0 {
            Err(Error::PlainIo(std::io::Error::last_os_error()))
        } else {
            Ok(ret)
        }
    }

    /// `HIDIOCGRAWINFO` + `HIDIOCGRAWNAME` + `HIDIOCGRAWPHYS`.
    ///
    /// The name and phys strings come back NUL-terminated; truncate at the
    /// first NUL rather than trusting the returned length alone.
    fn query_info(file: &File) -> Result<HidInfo> {
        let fd = file.as_fd();

        let mut raw = HidrawDevinfo::default();
        const REQ: u32 = ioc(IOC_READ, HID, 0x03, size_of::<HidrawDevinfo>() as u32);

        unsafe { Self::ioctl(fd, REQ, &raw mut raw)? };

        let name = Self::read_string_ioctl(fd, 0x04, "HID name")?;
        let phys = Self::read_string_ioctl(fd, 0x05, "HID phys")?;

        Ok(HidInfo { bus: BusType::from_hid_bus(raw.bustype), vendor: raw.vendor as u16, product: raw.product as u16, name, phys })
    }

    pub fn info(&self) -> &HidInfo {
        &self.info
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `HIDIOCSFEATURE` / `HIDIOCGFEATURE` — same shape, different `nr`.
    ///
    /// Both are `_IOWR`, so the buffer must be writable whichever way the
    /// data is actually flowing. Returns the byte count the kernel moved.
    fn feature_ioctl(fd: BorrowedFd<'_>, nr: u32, buf: &mut [u8]) -> Result<usize> {
        if buf.is_empty() {
            return Err(Error::invalid("report length", "A Feature Report needs at least a report ID byte",));
        }
        if buf.len() > IOC_SIZE_MAX as usize {
            return Err(Error::invalid("report length", "Exceeds the 14-bit ioctl size field",));
        }

        let req = ioc(IOC_WRITE | IOC_READ, HID, nr, buf.len() as u32);
        
        let n = unsafe { Self::ioctl(fd, req, buf.as_mut_ptr())? };
        
        Ok(n as usize)
    }

    /// Send a feature report. `data[0]` is the report ID — 0 when the device
    /// uses none.
    ///
    /// `HIDIOCSFEATURE` is declared read-write, so the buffer handed to the
    /// kernel must be writable even though nothing comes back through it.
    pub fn set_feature_report(&mut self, data: &[u8]) -> Result<usize> {
        let mut owned = data.to_vec();
        Self::feature_ioctl(self.file.as_fd(), 0x06, &mut owned)

    }

    /// Fetch a feature report. Set `buf[0]` to the report ID first; the
    /// report is written back into `buf`.
    pub fn get_feature_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        Self::feature_ioctl(self.file.as_fd(), 0x07, buf)
    }

    /// Write an output report. `data[0]` is the report ID.
    pub fn write_output_report(&mut self, data: &[u8]) -> Result<usize> {
        use std::io::Write;
        self.file.write(data).map_err(|err| Error::io(&self.path, err))
    }

    /// Read one input report. Blocks until the device sends something, so
    /// drivers that can stall want a timeout or non-blocking fd.
    pub fn read_input_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        use std::io::Read;
        self.file.read(buf).map_err(|err| Error::io(&self.path, err))
    }
}

/// Unit tests live outside `src/`, but compile as a child module of `hid` so
/// they can reach private items without widening the public API.
#[cfg(test)]
#[path = "../tests/unit/hid.rs"]
mod tests;
