// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![allow(incomplete_features)]
#![feature(const_mut_refs)]
#![feature(mixed_integer_ops)]
#![feature(trait_alias)]
#![feature(adt_const_params)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::similar_names)]
#![allow(clippy::if_not_else)]
#![allow(clippy::verbose_bit_mask)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::unsafe_derive_deserialize)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::match_on_vec_items)]
#![allow(clippy::invalid_upcast_comparisons)]

pub mod common;
pub mod debugger;
pub mod gga;
pub mod ggc;
pub mod numutil;
pub mod psx;
mod scheduler;
pub mod storage;

pub use common::System;

/// For debugging: If instruction-level tracing output should be printed.
const TRACING: bool = false;

/// Colour type used by the system's PPUs for image data.
/// This type is analogus to egui's `Color32`, which allows the GUI to
/// simply `mem::transmute` it without having to perform any explicit
/// conversion. Additionally, due to this approach the core crate does not need
/// to depend on the rather heavy egui.
pub type Colour = [u8; 4];
