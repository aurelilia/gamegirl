#![feature(duration_consts_float)]
#![feature(exclusive_range_pattern)]
#![feature(is_some_with)]
#![feature(mixed_integer_ops)]
#![feature(trait_alias)]

pub mod common;
pub mod debugger;
pub mod gga;
pub mod ggc;
pub mod numutil;
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
