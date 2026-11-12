//! A market data socket frame through the decode and the data conversion: no panic.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        nautilus_qfex::fuzzing::mds_message(text);
    }
});
