use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Apu {
    pub(super) buffer: Vec<f32>,
}
