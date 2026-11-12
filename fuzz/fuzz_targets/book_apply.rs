//! Arbitrary levels through the pulsed book: no panic, and a pulse applied twice leaves the
//! book as it was after the first time.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nautilus_qfex::fuzzing::{book_apply, decimal};

type Parts = (u32, u32, u32, bool, u8);

fuzz_target!(|input: (Vec<(Parts, Parts)>, Vec<(Parts, Parts)>, u64)| {
    let (bids, asks, sequence) = input;
    let levels = |side: Vec<(Parts, Parts)>| -> Vec<_> {
        let level = |(price, size)| (decimal(price), decimal(size));
        side.into_iter().map(level).collect()
    };
    book_apply(&levels(bids), &levels(asks), sequence);
});
