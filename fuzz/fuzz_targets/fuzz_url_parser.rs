#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    bzr::fuzz::parse_bugzilla_url(data);
});
