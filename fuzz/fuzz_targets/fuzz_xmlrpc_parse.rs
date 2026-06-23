#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    bzr::fuzz::parse_xmlrpc_response(data);
});
