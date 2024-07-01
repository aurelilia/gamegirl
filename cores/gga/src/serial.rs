#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Serial {
    pub rcnt: u16,
}

impl Default for Serial {
    fn default() -> Self {
        Self { rcnt: 0x8000 }
    }
}
