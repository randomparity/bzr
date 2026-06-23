#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let input = vec![data.to_string()];
    bzr::fuzz::parse_flags(&input);
});
