//! Arbitrary quantity and price through the order frames: no panic, valid JSON, and a number
//! of up to 15 significant digits reads back unchanged.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nautilus_qfex::fuzzing::{decimal, wire_format};

type Parts = (u32, u32, u32, bool, u8);

fuzz_target!(|input: (Parts, Parts)| {
    let (quantity, price) = input;
    wire_format(decimal(quantity), decimal(price));
});
