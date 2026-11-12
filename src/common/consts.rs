//! Venue-wide constants for the QFEX adapter.

use std::sync::LazyLock;

use nautilus_model::identifiers::Venue;
use ustr::Ustr;

/// Venue name used in instrument and account identifiers.
pub const QFEX: &str = "QFEX";

/// The QFEX venue.
pub static QFEX_VENUE: LazyLock<Venue> = LazyLock::new(|| Venue::new(Ustr::from(QFEX)));

/// Default HTTP request timeout.
pub const HTTP_TIMEOUT_SECS: u64 = 10;

/// REST contract version sent as `api_version` on every request (docs, 11 Nov 2026).
pub const QFEX_API_VERSION: &str = "2026-10-29";

/// Suffix that turns a venue symbol (`AAPL-USD`) into a Nautilus symbol (`AAPL-USD-PERP`).
pub const PERP_SUFFIX: &str = "-PERP";

/// Longest nonce the venue accepts.
pub const MAX_NONCE_LEN: usize = 100;
