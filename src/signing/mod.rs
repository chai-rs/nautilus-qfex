//! HMAC authentication: REST headers and the trade WebSocket `auth` frame.
//!
//! The signature is the lower-case hex HMAC-SHA256 of `{nonce}:{unix_ts}` keyed by the API
//! secret. The docs give no worked example, so the tests check the HMAC against a value
//! computed with `openssl`, not against a vector from the venue.

use std::{collections::HashMap, fmt::Debug, sync::Arc};

use nautilus_core::{UUID4, time::get_atomic_clock_realtime};
use nautilus_cryptography::signing::hmac_signature;
use serde_json::json;

use crate::common::{consts::MAX_NONCE_LEN, credential::QfexCredential};

pub const HEADER_PUBLIC_KEY: &str = "x-qfex-public-key";
pub const HEADER_SIGNATURE: &str = "x-qfex-hmac-signature";
pub const HEADER_NONCE: &str = "x-qfex-nonce";
pub const HEADER_TIMESTAMP: &str = "x-qfex-timestamp";

/// One signed authentication: valid once (the nonce) and for a few minutes (the timestamp).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QfexAuth {
    pub public_key: String,
    pub nonce: String,
    /// Unix seconds.
    pub unix_ts: u64,
    pub signature: String,
}

impl QfexAuth {
    /// Signs `nonce` and `unix_ts` with the credential.
    ///
    /// # Errors
    ///
    /// Returns an error if the nonce is empty or longer than the venue allows.
    pub fn sign(credential: &QfexCredential, nonce: &str, unix_ts: u64) -> anyhow::Result<Self> {
        let len = nonce.chars().count();
        anyhow::ensure!(
            (1..=MAX_NONCE_LEN).contains(&len),
            "nonce length {len} is outside 1..={MAX_NONCE_LEN}"
        );
        Ok(Self {
            public_key: credential.api_key().to_string(),
            nonce: nonce.to_string(),
            unix_ts,
            signature: hmac_signature(credential.api_secret(), &format!("{nonce}:{unix_ts}"))?,
        })
    }

    /// Returns the four REST authentication headers.
    #[must_use]
    pub fn headers(&self) -> HashMap<String, String> {
        HashMap::from([
            (HEADER_PUBLIC_KEY.to_string(), self.public_key.clone()),
            (HEADER_SIGNATURE.to_string(), self.signature.clone()),
            (HEADER_NONCE.to_string(), self.nonce.clone()),
            (HEADER_TIMESTAMP.to_string(), self.unix_ts.to_string()),
        ])
    }

    /// Returns the trade WebSocket `auth` frame.
    #[must_use]
    pub fn ws_message(&self) -> String {
        json!({
            "type": "auth",
            "params": {
                "hmac": {
                    "public_key": self.public_key,
                    "nonce": self.nonce,
                    "unix_ts": self.unix_ts,
                    "signature": self.signature,
                }
            }
        })
        .to_string()
    }
}

/// Source of `(nonce, unix seconds)` pairs.
pub type QfexNonceSource = Arc<dyn Fn() -> (String, u64) + Send + Sync>;

/// Signs with a fresh nonce and the current time on every call.
#[derive(Clone)]
pub struct QfexSigner {
    credential: QfexCredential,
    source: QfexNonceSource,
}

impl Debug for QfexSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(QfexSigner))
            .field("credential", &self.credential)
            .finish_non_exhaustive()
    }
}

impl QfexSigner {
    /// Creates a signer with a random UUID nonce (32 hex characters) and the system clock.
    #[must_use]
    pub fn new(credential: QfexCredential) -> Self {
        Self::with_source(
            credential,
            Arc::new(|| {
                let nonce = UUID4::new().to_string().replace('-', "");
                let unix_ts = get_atomic_clock_realtime().get_time_ns().as_u64() / 1_000_000_000;
                (nonce, unix_ts)
            }),
        )
    }

    /// Creates a signer with an explicit nonce and time source (tests).
    #[must_use]
    pub fn with_source(credential: QfexCredential, source: QfexNonceSource) -> Self {
        Self { credential, source }
    }

    #[must_use]
    pub fn api_key(&self) -> &str {
        self.credential.api_key()
    }

    /// Signs one request or one WebSocket login.
    ///
    /// # Errors
    ///
    /// Returns an error if the source gives an unusable nonce.
    pub fn auth(&self) -> anyhow::Result<QfexAuth> {
        let (nonce, unix_ts) = (self.source)();
        QfexAuth::sign(&self.credential, &nonce, unix_ts)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use rstest::rstest;

    use super::*;

    pub(crate) const API_KEY: &str = "dummy-public-key";
    pub(crate) const FAKE_SECRET: &str = "not-a-real-secret";
    pub(crate) const NONCE: &str = "0123456789abcdef0123456789abcdef";
    pub(crate) const UNIX_TS: u64 = 1_794_336_834;
    // printf '%s' '0123456789abcdef0123456789abcdef:1794336834' \
    //   | openssl dgst -sha256 -hmac 'not-a-real-secret'
    pub(crate) const SIGNATURE: &str =
        "9221b29f74bd807fd2015c7af36a8a3e26008a3b60186380ce9eb43919f53a8a";

    pub(crate) fn fixed_signer() -> QfexSigner {
        QfexSigner::with_source(
            QfexCredential::new(API_KEY, FAKE_SECRET),
            Arc::new(|| (NONCE.to_string(), UNIX_TS)),
        )
    }

    #[rstest]
    fn test_signature_matches_openssl() {
        let auth = fixed_signer().auth().unwrap();
        assert_eq!(auth.signature, SIGNATURE);
        assert_eq!(auth.public_key, API_KEY);
    }

    // RFC 4231 test case 2: checks the HMAC primitive against a published vector.
    #[rstest]
    fn test_hmac_primitive_matches_rfc_4231() {
        assert_eq!(
            hmac_signature("Jefe", "what do ya want for nothing?").unwrap(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[rstest]
    fn test_headers() {
        let headers = fixed_signer().auth().unwrap().headers();
        assert_eq!(headers.len(), 4);
        assert_eq!(headers[HEADER_PUBLIC_KEY], API_KEY);
        assert_eq!(headers[HEADER_SIGNATURE], SIGNATURE);
        assert_eq!(headers[HEADER_NONCE], NONCE);
        assert_eq!(headers[HEADER_TIMESTAMP], "1794336834");
    }

    #[rstest]
    fn test_ws_message_has_integer_timestamp() {
        let message = fixed_signer().auth().unwrap().ws_message();
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["type"], "auth");
        let hmac = &value["params"]["hmac"];
        assert_eq!(hmac["public_key"], API_KEY);
        assert_eq!(hmac["nonce"], NONCE);
        assert_eq!(hmac["unix_ts"], serde_json::json!(UNIX_TS));
        assert_eq!(hmac["signature"], SIGNATURE);
        assert!(!message.contains(FAKE_SECRET));
    }

    #[rstest]
    #[case("", false)]
    #[case("a", true)]
    #[case(&"a".repeat(100), true)]
    #[case(&"a".repeat(101), false)]
    fn test_nonce_length(#[case] nonce: &str, #[case] ok: bool) {
        let credential = QfexCredential::new(API_KEY, FAKE_SECRET);
        assert_eq!(QfexAuth::sign(&credential, nonce, UNIX_TS).is_ok(), ok);
    }

    #[rstest]
    fn test_default_source_gives_distinct_short_nonces_and_a_current_time() {
        let signer = QfexSigner::new(QfexCredential::new(API_KEY, FAKE_SECRET));
        let (a, b) = (signer.auth().unwrap(), signer.auth().unwrap());
        assert_ne!(a.nonce, b.nonce);
        assert_eq!(a.nonce.len(), 32);
        assert!(a.unix_ts > 1_700_000_000);
        assert!(!format!("{signer:?}").contains(FAKE_SECRET));
    }
}
