// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::vec::Vec;

use bincode::config;

/// Serialize an object that can be loaded with [deserialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(feature = "zstd")]
pub fn serialize<T: serde::Serialize>(thing: &T, with_zstd: bool) -> Vec<u8> {
    if with_zstd {
        let mut dest = Vec::new();
        let mut writer = zstd::stream::Encoder::new(&mut dest, 3).unwrap();
        bincode::serde::encode_into_std_write(thing, &mut writer, config::legacy()).unwrap();
        writer.finish().unwrap();
        dest
    } else {
        bincode::serde::encode_to_vec(thing, config::legacy()).unwrap()
    }
}

/// Deserialize an object that was made with [serialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(feature = "zstd")]
pub fn deserialize<T: serde::de::DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    if with_zstd {
        let mut decoder = zstd::stream::Decoder::new(state).unwrap();
        bincode::serde::decode_from_std_read(&mut decoder, config::legacy()).unwrap()
    } else {
        bincode::serde::decode_from_slice(state, config::legacy())
            .unwrap()
            .0
    }
}

/// Serialize an object that can be loaded with [deserialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(not(feature = "zstd"))]
pub fn serialize<T: serde::Serialize>(thing: &T, _with_zstd: bool) -> Vec<u8> {
    bincode::serde::encode_to_vec(thing, config::legacy()).unwrap()
}

/// Deserialize an object that was made with [serialize].
/// It is (optionally zstd-compressed) bincode.
#[cfg(not(feature = "zstd"))]
pub fn deserialize<T: serde::de::DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    bincode::serde::decode_from_slice(state, config::legacy()).unwrap()
}
