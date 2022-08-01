// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

/// Events the PPU generates.
#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum PpuEvent {
    /// Start of HBlank.
    HblankStart,
    /// Set HBlank flag in DISPSTAT (this is delayed by 46 cycles)
    SetHblank,
    /// End of HBlank, which is the start of the next scanline.
    HblankEnd,
}
