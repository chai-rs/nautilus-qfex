//! Arbitrary key, secret, nonce and timestamp through the HMAC signer: no panic, a signature of
//! 64 hex digits, and headers and an `auth` frame that carry the inputs unchanged.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (&str, &str, &str, u64)| {
    let (key, secret, nonce, unix_ts) = input;
    nautilus_qfex::fuzzing::auth(key, secret, nonce, unix_ts);
});
