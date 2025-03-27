use gamegirl::{common::common::options::SystemConfig, GameCart};

fn main() {
    let mut core = gamegirl::load_cart(
        GameCart {
            rom: include_bytes!("../../../bench.gb").to_vec(),
            save: None,
        },
        &SystemConfig::default(),
    )
    .unwrap();
    core.skip_bootrom();
    for _ in 0..200 {
        core.advance_delta(0.1);
    }
}
