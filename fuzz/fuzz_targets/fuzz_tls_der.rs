#![no_main]
use libfuzzer_sys::fuzz_target;

// INV-3: the best-effort DER issuer walkers must terminate without panicking
// on arbitrary certificate bytes.
fuzz_target!(|data: &[u8]| {
    bzr::fuzz::extract_issuer(data);
});
