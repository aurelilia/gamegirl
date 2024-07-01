use std::{iter, mem};

use common::Colour;
use eframe::egui::{Color32, TextureOptions};

macro_rules! hqx {
    ($mag:ident, $scale:expr, $size:expr, $input:expr) => {{
        let src: Vec<u32> = unsafe { mem::transmute($input) };
        let mut dst = Vec::with_capacity(src.len() * $scale * $scale);
        dst.extend(iter::repeat(0).take(src.len() * $scale * $scale));

        hqx::$mag(&src, &mut dst, $size[0], $size[1]);
        (
            unsafe { mem::transmute(dst) },
            [$size[0] * $scale, $size[1] * $scale],
            TextureOptions::NEAREST,
        )
    }};
}

#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Filter {
    Nearest,
    Linear,
    Hq2x,
    Hq3x,
    Hq4x,
}

pub fn apply_filter(
    input: Vec<Colour>,
    size: [usize; 2],
    filter: Filter,
) -> (Vec<Color32>, [usize; 2], TextureOptions) {
    match filter {
        Filter::Linear => (
            unsafe { mem::transmute(input) },
            size,
            TextureOptions::LINEAR,
        ),

        #[cfg(feature = "hqx")]
        Filter::Hq2x => hqx!(hq2x, 2, size, input),
        #[cfg(feature = "hqx")]
        Filter::Hq3x => hqx!(hq3x, 3, size, input),
        #[cfg(feature = "hqx")]
        Filter::Hq4x => hqx!(hq4x, 4, size, input),

        _ => (
            unsafe { mem::transmute(input) },
            size,
            TextureOptions::NEAREST,
        ),
    }
}
