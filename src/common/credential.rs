//! QFEX credential storage.

use std::fmt::Debug;

use nautilus_core::{env::get_or_env_var_opt, string::secret::REDACTED};
use zeroize::Zeroizing;

use crate::common::enums::QfexEnvironment;

/// Returns the `(API public key, API secret)` environment variable names.
#[must_use]
pub const fn credential_env_vars(environment: QfexEnvironment) -> (&'static str, &'static str) {
    match environment {
        QfexEnvironment::Mainnet => ("QFEX_API_KEY", "QFEX_API_SECRET"),
        QfexEnvironment::Uat => ("QFEX_UAT_API_KEY", "QFEX_UAT_API_SECRET"),
    }
}

/// API public key plus the HMAC secret.
///
/// The secret is wiped from memory on drop and never shown by `Debug`.
#[derive(Clone)]
pub struct QfexCredential {
    api_key: String,
    api_secret: Zeroizing<String>,
}

impl Debug for QfexCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(QfexCredential))
            .field("api_key", &self.api_key)
            .field("api_secret", &REDACTED)
            .finish()
    }
}

impl QfexCredential {
    /// Creates a new [`QfexCredential`].
    #[must_use]
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: Zeroizing::new(api_secret.into()),
        }
    }

    /// Resolves credentials from explicit values, falling back to the environment
    /// variables of `environment`. Returns `None` when neither value is available.
    ///
    /// # Errors
    ///
    /// Returns an error if only one of the two values is available.
    pub fn resolve(
        api_key: Option<String>,
        api_secret: Option<String>,
        environment: QfexEnvironment,
    ) -> anyhow::Result<Option<Self>> {
        let (key_var, secret_var) = credential_env_vars(environment);
        Self::from_resolved_values(
            get_or_env_var_opt(api_key, key_var),
            get_or_env_var_opt(api_secret, secret_var),
            environment,
        )
    }

    fn from_resolved_values(
        api_key: Option<String>,
        api_secret: Option<String>,
        environment: QfexEnvironment,
    ) -> anyhow::Result<Option<Self>> {
        let (key_var, secret_var) = credential_env_vars(environment);
        match (api_key, api_secret) {
            (Some(key), Some(secret)) => Ok(Some(Self::new(key, secret))),
            (None, None) => Ok(None),
            (Some(_), None) => {
                anyhow::bail!("incomplete QFEX credentials: {secret_var} is not set")
            }
            (None, Some(_)) => {
                anyhow::bail!("incomplete QFEX credentials: {key_var} is not set")
            }
        }
    }

    /// Returns the API public key.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns the HMAC secret. Do not log it.
    #[must_use]
    pub fn api_secret(&self) -> &str {
        &self.api_secret
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // Placeholder strings only: these tests never read the environment or a real key.
    const API_KEY: &str = "dummy-public-key";
    const FAKE_SECRET: &str = "not-a-real-secret";

    #[rstest]
    fn test_credential_env_vars() {
        assert_eq!(
            credential_env_vars(QfexEnvironment::Mainnet),
            ("QFEX_API_KEY", "QFEX_API_SECRET")
        );
        assert_eq!(
            credential_env_vars(QfexEnvironment::Uat),
            ("QFEX_UAT_API_KEY", "QFEX_UAT_API_SECRET")
        );
    }

    #[rstest]
    fn test_from_resolved_values() {
        let env = QfexEnvironment::Mainnet;
        let credential = QfexCredential::from_resolved_values(
            Some(API_KEY.to_string()),
            Some(FAKE_SECRET.to_string()),
            env,
        )
        .unwrap()
        .unwrap();
        assert_eq!(credential.api_key(), API_KEY);
        assert_eq!(credential.api_secret(), FAKE_SECRET);

        assert!(
            QfexCredential::from_resolved_values(None, None, env)
                .unwrap()
                .is_none()
        );
    }

    #[rstest]
    fn test_from_resolved_values_rejects_partial_values() {
        let env = QfexEnvironment::Uat;
        let missing_secret =
            QfexCredential::from_resolved_values(Some(API_KEY.to_string()), None, env)
                .unwrap_err()
                .to_string();
        assert!(missing_secret.contains("QFEX_UAT_API_SECRET"));

        let missing_api_key =
            QfexCredential::from_resolved_values(None, Some(FAKE_SECRET.to_string()), env)
                .unwrap_err()
                .to_string();
        assert!(missing_api_key.contains("QFEX_UAT_API_KEY"));
        assert!(!missing_api_key.contains(FAKE_SECRET));
    }

    #[rstest]
    fn test_debug_redacts_api_secret() {
        let debug = format!("{:?}", QfexCredential::new(API_KEY, FAKE_SECRET));
        assert!(debug.contains(API_KEY));
        assert!(debug.contains(REDACTED));
        assert!(!debug.contains(FAKE_SECRET));
    }
}
