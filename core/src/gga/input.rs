use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Input {
    pub regs: [u8; 4],
}
