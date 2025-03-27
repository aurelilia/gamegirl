#[cfg(feature = "std")]
pub fn since_unix() -> u64 {
    extern crate std;

    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(not(feature = "std"))]
pub fn since_unix() -> u64 {
    extern "C" {
        fn get_unix_time() -> u64;
    }
    unsafe { get_unix_time() }
}
