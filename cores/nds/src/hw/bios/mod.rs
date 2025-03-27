// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::vec::Vec;

use modular_bitfield::prelude::*;

// DraStic BIOS as found in MelonDS sources:
// https://github.com/melonDS-emu/melonDS/tree/5eadd67df6da429891fdfba02bf650f4fefe4ab6/freebios
// Thank you to it's developers!
pub const FREEBIOS7: &[u8] = include_bytes!("drastic_bios_arm7.bin");
pub const FREEBIOS9: &[u8] = include_bytes!("drastic_bios_arm9.bin");

#[derive(Debug)]
#[repr(packed)]
pub struct UserSettings {
    pub version: u16,
    pub color: u8,
    pub birthday_month: u8,
    pub birthday_day: u8,
    pub zero1: u8,
    pub nickname_utf16: [u16; 10],
    pub nickname_len: u16,
    pub message_utf16: [u16; 26],
    pub message_len: u16,
    pub alarm_hour: u8,
    pub alarm_minute: u8,
    pub unused: u16,
    pub alarm_enable: u8,
    pub zero2: u8,
    pub touch_calibration: [u8; 12],
    pub language: LanguageFlags,
    pub year: u8,
    pub zero3: u8,
    pub rtc_offset: u32,
    pub ff: u32,
    pub update_counter: u16,
    pub crc16: u16,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
pub struct LanguageFlags {
    language: B3,
    gba_on_lower: bool,
    backlight_level: B2,
    autostart_cart: bool,
    #[skip]
    __: B2,
    settings_lost: bool,
    settings_okay: B6,
}

impl UserSettings {
    pub fn get_bogus() -> UserSettings {
        let utf16 = "leela".encode_utf16();
        let utf16 = utf16.collect::<Vec<_>>();
        UserSettings {
            version: 5,
            color: 3,
            birthday_month: 11,
            birthday_day: 26,
            zero1: 0,
            nickname_utf16: utf16.clone().try_into().unwrap(),
            nickname_len: 5,
            message_utf16: utf16.try_into().unwrap(),
            message_len: 5,
            alarm_hour: 23,
            alarm_minute: 31,
            unused: 0,
            alarm_enable: 0,
            zero2: 0,
            touch_calibration: [0; 12],
            language: LanguageFlags::default()
                .with_language(1)
                .with_settings_okay(0x3F),
            year: 24,
            zero3: 0,
            rtc_offset: 0,
            ff: 0xFFFFFFFF,
            update_counter: 23,
            crc16: 0,
        }
    }
}
