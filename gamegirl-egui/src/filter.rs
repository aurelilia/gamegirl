use std::{mem, sync::Arc};

use common::Colour;
use eframe::egui::{Color32, TextureOptions};
use egui::ColorImage;

#[cfg(feature = "hqx")]
macro_rules! hqx {
    ($mag:ident, $scale:expr, $size:expr, $input:expr) => {{
        let src: Vec<u32> = unsafe { mem::transmute($input) };
        let mut dst = Vec::with_capacity(src.len() * $scale * $scale);
        dst.extend(std::iter::repeat(0).take(src.len() * $scale * $scale));

        hqx::$mag(&src, &mut dst, $size[0], $size[1]);
        (
            unsafe { mem::transmute(dst) },
            [$size[0] * $scale, $size[1] * $scale],
            TextureOptions::NEAREST,
        )
    }};
}

#[derive(Default)]
pub struct ScreenBuffer {
    pub buffer: Vec<Arc<ColorImage>>,
}

impl ScreenBuffer {
    pub fn next_frame(
        &mut self,
        size: [usize; 2],
        next: Vec<Colour>,
        filter: Filter,
        blend: Blend,
    ) -> (Arc<ColorImage>, TextureOptions) {
        let (pixels, size, filter) = apply_filter(next, size, filter);
        let new = ColorImage { pixels, size };
        match (blend, self.buffer.last_mut()) {
            (Blend::None, _) => (new.into(), filter),

            (Blend::Soften, Some(last)) => {
                let pixels = last
                    .pixels
                    .iter()
                    .zip(new.pixels.iter())
                    .map(|(a, b)| blend_pixel(*a, *b))
                    .collect();
                *last = new.into();
                (ColorImage { pixels, size }.into(), filter)
            }

            (Blend::Accumulate, Some(last)) => {
                let pixels = last
                    .pixels
                    .iter()
                    .zip(new.pixels.iter())
                    .map(|(a, b)| blend_pixel(*a, *b))
                    .collect();
                let img: Arc<ColorImage> = ColorImage { pixels, size }.into();
                *last = img.clone();
                (img, filter)
            }

            _ => {
                let arc: Arc<ColorImage> = new.into();
                self.buffer.push(arc.clone());
                (arc, filter)
            }
        }
    }
}

fn blend_pixel(mut a: Color32, b: Color32) -> Color32 {
    for i in 0..3 {
        a[i] = ((a[i] as u16 + b[i] as u16) / 2) as u8;
    }
    a
}

#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Filter {
    Nearest,
    Linear,
    Hq2x,
    Hq3x,
    Hq4x,
}

fn apply_filter(
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

#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Blend {
    None,
    Soften,
    Accumulate,
}
