use std::vec::Vec;

#[derive(Default)]
pub struct ScreenBuffer {
    pub buffer: Vec<Vec<[u8; 4]>>,
}

impl ScreenBuffer {
    pub fn next_frame(&mut self, next: Vec<[u8; 4]>, blend: Blend) -> Vec<[u8; 4]> {
        match (blend, self.buffer.last_mut()) {
            (Blend::None, _) => next,

            (Blend::Soften, Some(last)) => {
                let pixels = last
                    .iter()
                    .zip(next.iter())
                    .map(|(a, b)| blend_pixel(*a, *b))
                    .collect();
                *last = next.into();
                pixels
            }

            (Blend::Accumulate, Some(last)) => {
                let pixels: Vec<[u8; 4]> = last
                    .iter()
                    .zip(next.iter())
                    .map(|(a, b)| blend_pixel(*a, *b))
                    .collect();
                *last = pixels.clone();
                pixels
            }

            _ => {
                self.buffer.push(next.clone());
                next
            }
        }
    }
}

fn blend_pixel(mut a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    for i in 0..3 {
        a[i] = ((a[i] as u16 + b[i] as u16) / 2) as u8;
    }
    a
}

#[derive(Copy, Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Blend {
    None,
    Soften,
    Accumulate,
}
