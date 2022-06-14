#![feature(duration_consts_float)]
#![feature(exclusive_range_pattern)]
#![feature(mixed_integer_ops)]

pub mod common;
pub mod debugger;
pub mod gga;
pub mod ggc;
pub mod numutil;
pub mod storage;

pub use common::System;

pub type Colour = [u8; 4];
