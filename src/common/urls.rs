//! Base URLs per QFEX environment.

use crate::common::enums::QfexEnvironment;

/// Returns the REST base URL for the environment.
#[must_use]
pub const fn qfex_http_base_url(environment: QfexEnvironment) -> &'static str {
    match environment {
        QfexEnvironment::Mainnet => "https://api.qfex.com",
        QfexEnvironment::Uat => "https://api.qfex.io",
    }
}

/// Returns the public market data WebSocket URL for the environment.
#[must_use]
pub const fn qfex_mds_ws_url(environment: QfexEnvironment) -> &'static str {
    match environment {
        QfexEnvironment::Mainnet => "wss://mds.qfex.com",
        // The docs name only the REST and trade hosts of UAT: this host is assumed.
        QfexEnvironment::Uat => "wss://mds.qfex.io",
    }
}

/// Returns the trade (order entry) WebSocket URL for the environment.
#[must_use]
pub const fn qfex_trade_ws_url(environment: QfexEnvironment) -> &'static str {
    match environment {
        QfexEnvironment::Mainnet => "wss://trade.qfex.com",
        QfexEnvironment::Uat => "wss://trade.qfex.io",
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(
        QfexEnvironment::Mainnet,
        "https://api.qfex.com",
        "wss://mds.qfex.com",
        "wss://trade.qfex.com"
    )]
    #[case(
        QfexEnvironment::Uat,
        "https://api.qfex.io",
        "wss://mds.qfex.io",
        "wss://trade.qfex.io"
    )]
    fn test_urls_per_environment(
        #[case] environment: QfexEnvironment,
        #[case] http: &str,
        #[case] mds: &str,
        #[case] trade: &str,
    ) {
        assert_eq!(qfex_http_base_url(environment), http);
        assert_eq!(qfex_mds_ws_url(environment), mds);
        assert_eq!(qfex_trade_ws_url(environment), trade);
    }
}
