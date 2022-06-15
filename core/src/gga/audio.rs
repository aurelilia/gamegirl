use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Apu {
    pub buffer: Vec<f32>,
}
