fn main() {
    let mut core = dynacore::new_core(include_bytes!("../../../bench.gb").to_vec());
    for _ in 0..1000 {
        core.advance_delta(0.1);
    }
}
