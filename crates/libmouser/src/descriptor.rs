//! Walking HID report descriptors.
//!
//! One physical device presents several hidraw nodes with identical
//! bus/vid/pid, so [`crate::db::DeviceDb`] cannot tell them apart and a
//! driver has to. The report descriptor is the only thing that distinguishes
//! them, and both drivers ask the same two questions of it:
//!
//! | driver | wants |
//! |---|---|
//! | `hidpp20` | usage page `0xFF00` **and** report `0x10` |
//! | `c18` | usage page `0xFF00` **and no report IDs at all** |
//!
//! Observed on a G502 HERO and a Zelotes C-18:
//!
//! ```text
//! C-18  iface 0   pages 0x01 0x09 0x0c             reports —
//! C-18  iface 1   pages 0x01 0x07 0x0c 0xff01      reports 1 2 3 6
//! C-18  iface 2   pages 0xff00                     reports —
//! G502  iface 0   pages 0x01 0x09 0x0c             reports —
//! G502  iface 1   pages 0x01 0x07 0x0c 0xff00      reports 1 3 4 0x10 0x11
//! ```
//!
//! Two things that table makes obvious:
//!
//! * The G502's HID++ node *also* carries `0xFF00`, so the page alone does
//!   not identify the C-18 — the empty report-ID set is what separates them.
//! * The C-18's keyboard interface uses `0xFF01`, one off from `0xFF00`. A
//!   range check for "some vendor page" matches both. Compare exactly.
//!
//! ## Why a walker and not a byte search
//!
//! Searching for `06 00 ff` as a subsequence mostly works, and is what
//! `tests/hardware.rs` does. But descriptors contain *data* bytes as well as
//! item headers, and nothing stops a payload byte sequence from looking like
//! a Usage Page item. A walker only ever reads at real item boundaries, so it
//! cannot be fooled that way.
//!
//! The test helpers stay as naive byte searches on purpose: they are then an
//! independent check on this module rather than the same logic written twice.
//!
//! ## Item format
//!
//! A descriptor is a flat sequence of items. Each starts with a prefix byte:
//!
//! ```text
//! bits 0..=1   bSize   data byte count — 0, 1, 2 or 3
//! bits 2..=3   bType   0 Main, 1 Global, 2 Local, 3 Reserved
//! bits 4..=7   bTag
//! ```
//!
//! Then `bSize` data bytes, **little-endian**.
//!
//! Nothing here cares about Main or Local items, but they still have to be
//! *skipped correctly* — misread one length and every item after it is
//! garbage read at the wrong offset.

use crate::error::{Error, Result};

/// `bSize` of 3 means **four** data bytes, not three. The single most likely
/// way to desync a walk.
const SIZE_OF_LONG_FORM: usize = 4;

/// A prefix byte of `0xFE` introduces a long item: `FE`, then a data length,
/// then a tag, then that many data bytes. Vanishingly rare in practice, but a
/// walker that does not recognise it reads the length byte as a prefix and
/// everything after is nonsense.
const LONG_ITEM_PREFIX: u8 = 0xFE;

/// `bType` values.
const TYPE_GLOBAL: u8 = 1;

/// Global `bTag` for Usage Page.
const TAG_USAGE_PAGE: u8 = 0x0;
/// Global `bTag` for Report ID.
const TAG_REPORT_ID: u8 = 0x8;

/// One item from a report descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    /// `bType`: 0 Main, 1 Global, 2 Local, 3 Reserved.
    item_type: u8,
    /// `bTag`, 0..=15.
    tag: u8,
    /// The data bytes, little-endian and zero-extended. A long item's payload
    /// is skipped rather than decoded, so this is 0 for one of those.
    value: u32,
}

/// Split a descriptor into items.
///
/// Returns [`Error::invalid`] on a truncated item — the last prefix claiming
/// more data bytes than remain. That must be an error rather than a panic:
/// these bytes come from the kernel, and a short descriptor is the kind of
/// thing a malfunctioning device produces.
///
/// `bSize` of 3 means [`SIZE_OF_LONG_FORM`] bytes. Handle
/// [`LONG_ITEM_PREFIX`] before decoding the prefix normally, since `0xFE`
/// would otherwise parse as a Reserved item with 2 data bytes and leave the
/// walk one byte out.
fn walk(bytes: &[u8]) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == LONG_ITEM_PREFIX {
            let header = bytes.get(i + 1..i + 3).ok_or_else(|| {
                Error::invalid("report descriptor", 
                    format!("long item at offset {i} ends before its size and tag"),
                )
            })?;

            let data_size = usize::from(header[0]);
            let tag = header[1];
            let end = i + 3 + data_size;

            if end > bytes.len() {
                return Err(Error::invalid("report descriptor", 
                    format!("long item at offset {i} claims {data_size} data bytes. {} remain", bytes.len() - i - 3),
                ));
            }

            items.push(Item { item_type: 3, tag, value: 0 });
            i = end;
            continue;
        }
        let prefix = bytes[i];
        let size_field = prefix & 0x03;
        let item_type = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;

        let n = if size_field == 3 { SIZE_OF_LONG_FORM } else { usize::from(size_field) };

        let data = bytes.get(i + 1..i + 1 + n).ok_or_else(|| {
                Error::invalid("report descriptor", format!("item at offset {i} claims {n} data byte, {} remain", bytes.len() - i - 1),
            )
        })?;

        let mut buf = [0u8; SIZE_OF_LONG_FORM];
        buf[..n].copy_from_slice(data);
        let value = u32::from_le_bytes(buf);

        items.push(Item { item_type, tag, value });
        i += 1 + n;
    }

    Ok(items)
}

/// What a report descriptor says about pages and reports.
///
/// Deliberately not a full parse. Collections, usages, report sizes and
/// counts are all walked past: no driver here needs them, and the ones that
/// might (a real HID consumer) would want a different type anyway.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportDescriptor {
    /// Usage pages, in declaration order, deduplicated.
    ///
    /// Order is kept because "the first page declared" is a question drivers
    /// ask — it is upstream libratbag's `usage_page(0)`.
    usage_pages: Vec<u16>,
    /// Report IDs, ascending and deduplicated.
    ///
    /// Empty means the device uses unnumbered reports, which is itself the
    /// identifying fact for the C-18's configuration interface.
    report_ids: Vec<u8>,
}

impl ReportDescriptor {
    /// Walk `bytes` and collect the Global Usage Page and Report ID items.
    ///
    /// Both are *Global* items, so each one stays in effect until the next of
    /// its kind — but since this only collects the set of values declared,
    /// that persistence does not need modelling.
    ///
    /// A Usage Page may be declared with 1, 2 or (legally, though unseen
    /// here) 4 data bytes, so the value needs narrowing to `u16`. A Report ID
    /// is one byte; a larger value is malformed and worth rejecting rather
    /// than truncating, since truncating could invent a plausible ID.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let items = walk(bytes)?;
        todo!()
    }

    pub fn usage_pages(&self) -> &[u16] {
        &self.usage_pages
    }

    pub fn report_ids(&self) -> &[u8] {
        &self.report_ids
    }

    /// Exact match. A range test for "any vendor page" would accept the
    /// C-18's `0xFF01` keyboard interface as well as its `0xFF00` config one.
    pub fn has_usage_page(&self, page: u16) -> bool {
        self.usage_pages.contains(&page)
    }

    pub fn has_report(&self, id: u8) -> bool {
        self.report_ids.contains(&id)
    }

    /// True when the device uses unnumbered reports.
    ///
    /// For an unnumbered report the kernel takes byte 0 of a write as the
    /// report number and strips it when it is zero — which is why the C-18's
    /// two transports need opposite handling of that byte.
    pub fn is_unnumbered(&self) -> bool {
        self.report_ids.is_empty()
    }
}

/// Unit tests live in `tests/unit/`, compiled in as a child module so they
/// can reach the private `walk` and `Item`.
#[cfg(test)]
#[path = "../tests/unit/descriptor.rs"]
mod tests;
