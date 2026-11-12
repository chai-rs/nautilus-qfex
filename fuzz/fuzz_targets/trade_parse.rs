//! A trade socket frame (order responses, fills, balances, errors) through the decode and
//! the order dispatch of a session with one open order: no panic.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        nautilus_qfex::fuzzing::trade_message(text);
    }
});
