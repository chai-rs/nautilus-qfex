//! A REST body through every response model and its conversion: no panic.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        nautilus_qfex::fuzzing::rest_response(text);
    }
});
