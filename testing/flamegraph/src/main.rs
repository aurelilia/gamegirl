use gamegirl::common::common::options::SystemConfig;

fn main() {
    let mut core = gamegirl::load_cart(
        include_bytes!("../../../bench.gb").to_vec(),
        None,
        &SystemConfig::default(),
        None,
        0,
    )
    .unwrap();
    for _ in 0..200 {
        core.advance_delta(0.1);
    }
}
