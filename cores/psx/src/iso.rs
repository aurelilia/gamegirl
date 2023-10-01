#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Iso {
    pub raw: Vec<u8>,
}

impl Iso {
    pub fn title(&self) -> String {
        "IDK DUDE".into()
    }
}
