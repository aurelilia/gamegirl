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

pub type Colour = [u8; 4];
