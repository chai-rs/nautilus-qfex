//! HTTP client for the QFEX REST endpoints.
//!
//! Public endpoints need no credentials. The `/user/*` endpoints need a [`QfexSigner`]; every
//! request then carries a fresh nonce and signature.

use std::{collections::HashMap, num::NonZeroU32};

use nautilus_core::{UnixNanos, time::get_atomic_clock_realtime};
use nautilus_model::{
    data::{Bar, BarType, FundingRateUpdate, OrderBookDeltas},
    instruments::{Instrument, InstrumentAny},
};
use nautilus_network::{
    http::{
        HttpClient, HttpRedirectPolicy, HttpResponse, Method, create_standard_nautilus_headers,
        parse_retry_after,
    },
    ratelimiter::quota::Quota,
    retry::{RetryManager, create_http_retry_manager},
};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use ustr::Ustr;

use crate::{
    book::parse_book_deltas,
    common::{
        consts::QFEX_API_VERSION,
        enums::{QfexBarInterval, QfexEnvironment, QfexSymbolStatus},
        parse::venue_symbol_of,
        urls::qfex_http_base_url,
    },
    http::{
        error::{
            QfexHttpError, QfexHttpResult, create_qfex_http_timeout_error,
            should_retry_qfex_http_error,
        },
        models::{
            QfexCandles, QfexFees, QfexFundingRates, QfexHistoricOrders, QfexLeverageRequest,
            QfexOrderBook, QfexProblem, QfexRefData, QfexSymbol, QfexTrades, QfexUserPositions,
        },
        parse::{parse_bar, parse_funding_rate, parse_instrument},
    },
    signing::QfexSigner,
};

const HEADER_RETRY_AFTER: &str = "retry-after";
const REST_BUCKET: &str = "qfex:rest";

/// Largest `limit` of the paged `/user/*` endpoints (OpenAPI, 11 Nov 2026).
pub const MAX_PAGE_LIMIT: u32 = 1_000;

/// Funding is paid every hour.
pub const FUNDING_INTERVAL_MINUTES: u16 = 60;

/// Bars or funding records requested when the caller gives no start and no limit.
const DEFAULT_HISTORY_LEN: u64 = 500;

type QueryParams = Vec<(&'static str, String)>;

/// Formats a time the way the venue takes it: `2026-11-10T15:04:00Z`.
fn iso(ts: UnixNanos) -> String {
    format!("{:.0}", ts.to_datetime_utc())
}

/// Start and end of a history request, and how many records to keep.
///
/// The venue requires both ends. A missing end is now; a missing start is `limit` (or
/// [`DEFAULT_HISTORY_LEN`]) steps before the end.
fn history_range(
    start: Option<UnixNanos>,
    end: Option<UnixNanos>,
    limit: Option<usize>,
    step_ns: u64,
    now: UnixNanos,
) -> (UnixNanos, UnixNanos) {
    let end = end.unwrap_or(now);
    let steps = limit.map_or(DEFAULT_HISTORY_LEN, |limit| limit as u64);
    let start = start.unwrap_or_else(|| {
        UnixNanos::from(end.as_u64().saturating_sub(steps.saturating_mul(step_ns)))
    });
    (start, end)
}

/// Keeps the newest `limit` records of an ascending list.
fn keep_last<T>(mut records: Vec<T>, limit: Option<usize>) -> Vec<T> {
    if let Some(limit) = limit
        && records.len() > limit
    {
        records.drain(..records.len() - limit);
    }
    records
}

/// Filter of `GET /user/trade` and `GET /user/historic-orders`.
#[derive(Debug, Clone, Default)]
pub struct QfexUserQuery {
    pub symbol: Option<String>,
    /// Venue order id: `exchange_order_id` for trades, `order_id` for orders.
    pub order_id: Option<String>,
    pub start: Option<UnixNanos>,
    pub end: Option<UnixNanos>,
    /// 1 to [`MAX_PAGE_LIMIT`]; the venue default is 100.
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl QfexUserQuery {
    fn params(&self, order_id_key: &'static str) -> QueryParams {
        let mut params = Vec::new();
        params.extend(self.symbol.clone().map(|v| ("symbol", v)));
        params.extend(self.order_id.clone().map(|v| (order_id_key, v)));
        params.extend(self.start.map(|v| ("start", iso(v))));
        params.extend(self.end.map(|v| ("end", iso(v))));
        params.extend(
            self.limit
                .map(|v| ("limit", v.min(MAX_PAGE_LIMIT).to_string())),
        );
        params.extend(self.offset.map(|v| ("offset", v.to_string())));
        params
    }
}

/// QFEX REST client: one method per endpoint, plus requests that return Nautilus data.
#[derive(Clone, Debug)]
pub struct QfexHttpClient {
    base_url: String,
    api_version: String,
    client: HttpClient,
    retry_manager: RetryManager<QfexHttpError>,
    signer: Option<QfexSigner>,
}

impl QfexHttpClient {
    /// Creates a new [`QfexHttpClient`]. Without a `signer` only the public endpoints work.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be created.
    pub fn new(
        environment: QfexEnvironment,
        base_url: Option<String>,
        api_version: Option<String>,
        timeout_secs: u64,
        proxy_url: Option<String>,
        signer: Option<QfexSigner>,
    ) -> QfexHttpResult<Self> {
        let base_url = base_url
            .unwrap_or_else(|| qfex_http_base_url(environment).to_string())
            .trim_end_matches('/')
            .to_string();
        // shortcut: the docs give no REST rate limit, so 10 requests per second is a guess
        // on the safe side; set the documented quota once QFEX publishes one.
        const REQUESTS_PER_SECOND: NonZeroU32 = NonZeroU32::new(10).unwrap();

        Ok(Self {
            base_url,
            api_version: api_version.unwrap_or_else(|| QFEX_API_VERSION.to_string()),
            client: HttpClient::builder()
                .redirect_policy(HttpRedirectPolicy::Reject)
                .header_keys(vec![HEADER_RETRY_AFTER.to_string()])
                .headers(create_standard_nautilus_headers().into_iter().collect())
                .maybe_default_quota(Quota::per_second(REQUESTS_PER_SECOND))
                .timeout_secs(timeout_secs)
                .maybe_proxy_url(proxy_url)
                .build()?,
            retry_manager: create_http_retry_manager(),
            signer,
        })
    }

    /// Returns the configured REST base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Whether the client can call the `/user/*` endpoints.
    #[must_use]
    pub const fn has_credentials(&self) -> bool {
        self.signer.is_some()
    }

    /// Overrides the retry manager. Intended for mock-server tests that need a shorter
    /// backoff than the default.
    pub fn set_retry_manager(&mut self, retry_manager: RetryManager<QfexHttpError>) {
        self.retry_manager = retry_manager;
    }

    /// Calls `GET /refdata`: every symbol of the venue.
    pub async fn get_refdata(&self) -> QfexHttpResult<QfexRefData> {
        self.get("/refdata", Vec::new(), false).await
    }

    /// Calls `GET /md/orderbook/{ticker_id}`.
    pub async fn get_order_book(&self, symbol: &str) -> QfexHttpResult<QfexOrderBook> {
        let endpoint = format!("/md/orderbook/{}", path_segment(symbol)?);
        self.get(&endpoint, Vec::new(), false).await
    }

    /// Calls `GET /candles/{symbol}`. The venue returns the newest candle first.
    pub async fn get_candles(
        &self,
        symbol: &str,
        resolution: QfexBarInterval,
        start: UnixNanos,
        end: UnixNanos,
    ) -> QfexHttpResult<QfexCandles> {
        let endpoint = format!("/candles/{}", path_segment(symbol)?);
        let params = vec![
            ("resolution", resolution.as_ref().to_string()),
            ("fromISO", iso(start)),
            ("toISO", iso(end)),
        ];
        self.get(&endpoint, params, false).await
    }

    /// Calls `GET /funding/{symbol}`.
    pub async fn get_funding(
        &self,
        symbol: &str,
        interval_minutes: u16,
        start: UnixNanos,
        end: UnixNanos,
    ) -> QfexHttpResult<QfexFundingRates> {
        let endpoint = format!("/funding/{}", path_segment(symbol)?);
        let params = vec![
            ("intervalMinutes", interval_minutes.to_string()),
            ("fromISO", iso(start)),
            ("toISO", iso(end)),
        ];
        self.get(&endpoint, params, false).await
    }

    /// Calls `GET /user/positions`: the balance and every position (authenticated).
    pub async fn get_user_positions(&self) -> QfexHttpResult<QfexUserPositions> {
        self.get("/user/positions", Vec::new(), true).await
    }

    /// Calls `GET /user/trade`: fills of the account (authenticated).
    pub async fn get_user_trades(&self, query: &QfexUserQuery) -> QfexHttpResult<QfexTrades> {
        self.get("/user/trade", query.params("exchange_order_id"), true)
            .await
    }

    /// Calls `GET /user/historic-orders`: orders in a final state (authenticated).
    pub async fn get_historic_orders(
        &self,
        query: &QfexUserQuery,
    ) -> QfexHttpResult<QfexHistoricOrders> {
        self.get("/user/historic-orders", query.params("order_id"), true)
            .await
    }

    /// Calls `GET /user/fees` (authenticated).
    pub async fn get_fees(&self) -> QfexHttpResult<QfexFees> {
        self.get("/user/fees", Vec::new(), true).await
    }

    /// Calls `POST /user/leverage` (authenticated). Not retried: it changes the account.
    pub async fn set_leverage(&self, symbol: &str, leverage: Decimal) -> QfexHttpResult<()> {
        let body = serde_json::to_vec(&QfexLeverageRequest {
            symbol: Ustr::from(symbol),
            leverage,
        })?;
        let mut headers = self.auth_headers()?;
        headers.insert("content-type".to_string(), "application/json".to_string());
        let params: QueryParams = vec![("api_version", self.api_version.clone())];
        let response = self
            .client
            .request_with_params(
                Method::POST,
                self.url("/user/leverage"),
                Some(&params),
                Some(headers),
                Some(body),
                None,
                Some(Self::rate_limit_keys()),
            )
            .await?;
        // The response body is not used: only the status counts.
        Self::check_status(&response)
    }

    /// Requests every listed instrument. Delisted symbols and symbols that do not parse are
    /// left out.
    pub async fn request_instruments(&self) -> QfexHttpResult<Vec<InstrumentAny>> {
        let refdata = self.get_refdata().await?;
        let ts_init = get_atomic_clock_realtime().get_time_ns();
        let mut instruments = Vec::with_capacity(refdata.data.len());
        for item in refdata.data {
            let symbol = match serde_json::from_value::<QfexSymbol>(item) {
                Ok(symbol) => symbol,
                Err(e) => {
                    log::warn!("Skipping unreadable QFEX symbol: {e}");
                    continue;
                }
            };
            if symbol.status == QfexSymbolStatus::Delisted {
                continue;
            }
            match parse_instrument(&symbol, ts_init) {
                Ok(instrument) => instruments.push(instrument),
                Err(e) => log::warn!("Skipping QFEX symbol {}: {e}", symbol.symbol),
            }
        }
        Ok(instruments)
    }

    /// Requests the current order book as snapshot deltas.
    pub async fn request_book_snapshot(
        &self,
        instrument: &InstrumentAny,
    ) -> QfexHttpResult<OrderBookDeltas> {
        let book = self
            .get_order_book(venue_symbol_of(&instrument.id())?)
            .await?;
        let ts_init = get_atomic_clock_realtime().get_time_ns();
        let ts_event = UnixNanos::from(book.timestamp.saturating_mul(1_000_000));
        Ok(parse_book_deltas(
            instrument, &book.bids, &book.asks, 0, ts_event, ts_init,
        )?)
    }

    /// Requests bars, oldest first.
    pub async fn request_bars(
        &self,
        bar_type: BarType,
        instrument: &InstrumentAny,
        start: Option<UnixNanos>,
        end: Option<UnixNanos>,
        limit: Option<usize>,
    ) -> QfexHttpResult<Vec<Bar>> {
        let interval = QfexBarInterval::try_from(&bar_type)
            .map_err(|e| QfexHttpError::InvalidRequest(e.to_string()))?;
        let ts_init = get_atomic_clock_realtime().get_time_ns();
        let (start, end) = history_range(start, end, limit, interval.as_nanos(), ts_init);
        // shortcut: one request per call (the docs give no cap on candles per response),
        // page backwards from `end` if a long range ever comes back short.
        let candles = self
            .get_candles(venue_symbol_of(&instrument.id())?, interval, start, end)
            .await?;
        let mut bars = candles
            .candles
            .iter()
            .map(|candle| parse_bar(candle, bar_type, instrument, ts_init))
            .collect::<anyhow::Result<Vec<_>>>()?;
        bars.sort_by_key(|bar| bar.ts_event);
        Ok(keep_last(bars, limit))
    }

    /// Requests hourly funding rates, oldest first. The window that is still open is left
    /// out: its rate is not final.
    pub async fn request_funding_rates(
        &self,
        instrument: &InstrumentAny,
        start: Option<UnixNanos>,
        end: Option<UnixNanos>,
        limit: Option<usize>,
    ) -> QfexHttpResult<Vec<FundingRateUpdate>> {
        let ts_init = get_atomic_clock_realtime().get_time_ns();
        let step_ns = u64::from(FUNDING_INTERVAL_MINUTES) * 60_000_000_000;
        let (start, end) = history_range(start, end, limit, step_ns, ts_init);
        let funding = self
            .get_funding(
                venue_symbol_of(&instrument.id())?,
                FUNDING_INTERVAL_MINUTES,
                start,
                end,
            )
            .await?;
        let mut rates = funding
            .data
            .iter()
            .map(|record| parse_funding_rate(record, instrument.id(), ts_init))
            .collect::<anyhow::Result<Vec<_>>>()?;
        rates.retain(|rate| rate.ts_event <= ts_init);
        rates.sort_by_key(|rate| rate.ts_event);
        Ok(keep_last(rates, limit))
    }

    fn auth_headers(&self) -> QfexHttpResult<HashMap<String, String>> {
        let signer = self.signer.as_ref().ok_or_else(|| {
            QfexHttpError::InvalidRequest("this endpoint needs API credentials".to_string())
        })?;
        let auth = signer
            .auth()
            .map_err(|e| QfexHttpError::InvalidRequest(e.to_string()))?;
        Ok(auth.headers())
    }

    async fn get<T>(
        &self,
        endpoint: &str,
        mut params: QueryParams,
        signed: bool,
    ) -> QfexHttpResult<T>
    where
        T: DeserializeOwned,
    {
        params.push(("api_version", self.api_version.clone()));
        let url = self.url(endpoint);
        let params = &params;
        self.retry_manager
            .invocation(
                endpoint,
                || {
                    let url = url.clone();

                    async move {
                        // Signed again on every attempt: a nonce is good for one request.
                        let headers = signed.then(|| self.auth_headers()).transpose()?;
                        let response = self
                            .client
                            .request_with_params(
                                Method::GET,
                                url,
                                Some(params),
                                headers,
                                None,
                                None,
                                Some(Self::rate_limit_keys()),
                            )
                            .await?;
                        Self::check_status(&response)?;
                        Ok(serde_json::from_slice(&response.body)?)
                    }
                },
                should_retry_qfex_http_error,
                |e| create_qfex_http_timeout_error(e.to_string()),
            )
            .retry_delay(&QfexHttpError::retry_after)
            .execute()
            .await
    }

    fn check_status(response: &HttpResponse) -> QfexHttpResult<()> {
        if response.status.is_success() {
            return Ok(());
        }
        let retry_after = response.headers.get(HEADER_RETRY_AFTER).and_then(|value| {
            let now = get_atomic_clock_realtime().get_time_ns().to_datetime_utc();
            parse_retry_after(value, now)
        });
        let status = response.status.as_u16();
        let body = String::from_utf8_lossy(&response.body).to_string();

        // Status first: an error body must not override the retry decision for 5xx and 429.
        if status >= 500 {
            return Err(QfexHttpError::Http {
                status,
                body,
                retry_after,
            });
        }
        if status == 429 {
            return Err(QfexHttpError::RateLimit {
                message: body,
                retry_after,
            });
        }
        match serde_json::from_slice::<QfexProblem>(&response.body) {
            Ok(problem) if problem.title.is_some() || problem.detail.is_some() => {
                Err(QfexHttpError::Venue {
                    code: problem.title.unwrap_or_else(|| status.to_string()),
                    message: problem.detail.unwrap_or_default(),
                })
            }
            _ => Err(QfexHttpError::Http {
                status,
                body,
                retry_after,
            }),
        }
    }

    fn url(&self, endpoint: &str) -> String {
        format!("{}{}", self.base_url, endpoint)
    }

    // One bucket for every request: the client-side quota is per client, not per route.
    fn rate_limit_keys() -> Vec<String> {
        vec![REST_BUCKET.to_string()]
    }
}

/// Checks a symbol before it becomes part of a URL path.
fn path_segment(symbol: &str) -> QfexHttpResult<&str> {
    let valid = !symbol.is_empty()
        && symbol
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if valid && symbol != "." && symbol != ".." {
        Ok(symbol)
    } else {
        Err(QfexHttpError::InvalidRequest(format!(
            "invalid symbol {symbol:?}"
        )))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Router,
        body::Bytes,
        extract::RawQuery,
        http::{HeaderMap, StatusCode},
        routing::{get, post},
    };
    use nautilus_network::retry::RetryConfig;
    use rstest::rstest;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::{
        http::{models::tests::fixture_json, parse::tests::instrument},
        signing::{
            HEADER_NONCE, HEADER_PUBLIC_KEY, HEADER_SIGNATURE, HEADER_TIMESTAMP,
            tests::{API_KEY, NONCE, SIGNATURE, fixed_signer},
        },
    };

    pub(crate) async fn serve(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{address}")
    }

    /// Client for the mock server with a backoff of one millisecond.
    pub(crate) fn client(base_url: String, max_retries: u32, signed: bool) -> QfexHttpClient {
        let mut client = QfexHttpClient::new(
            QfexEnvironment::Mainnet,
            Some(base_url),
            None,
            5,
            None,
            signed.then(fixed_signer),
        )
        .unwrap();
        client.set_retry_manager(RetryManager::new(RetryConfig {
            max_retries,
            initial_delay_ms: 1,
            max_delay_ms: 1,
            backoff_factor: 1.0,
            jitter_ms: 0,
            operation_timeout_ms: Some(5_000),
            immediate_first: false,
            max_elapsed_ms: None,
        }));
        client
    }

    /// Route that counts its calls and always answers with `status` and `body`.
    fn counted(
        status: StatusCode,
        body: &'static str,
    ) -> (Arc<AtomicUsize>, axum::routing::MethodRouter) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let handler = move || {
            counter.fetch_add(1, Ordering::SeqCst);
            async move { (status, body) }
        };
        (hits, get(handler.clone()).post(handler))
    }

    #[rstest]
    fn test_iso_and_history_range() {
        let end = UnixNanos::from(1_794_323_040_000_000_000);
        assert_eq!(iso(end), "2026-11-10T15:04:00Z");
        assert_eq!(
            iso(UnixNanos::from(end.as_u64() + 999_999_999)),
            "2026-11-10T15:04:00Z"
        );

        let minute = 60_000_000_000;
        let (start, to) = history_range(None, Some(end), Some(2), minute, UnixNanos::from(1));
        assert_eq!(
            (start, to),
            (UnixNanos::from(end.as_u64() - 2 * minute), end)
        );
        // No end: now. No limit: the default length. Never below zero.
        let (start, to) = history_range(None, None, None, minute, UnixNanos::from(7));
        assert_eq!((start, to), (UnixNanos::default(), UnixNanos::from(7)));

        assert_eq!(keep_last(vec![1, 2, 3], Some(2)), vec![2, 3]);
        assert_eq!(keep_last(vec![1, 2, 3], None), vec![1, 2, 3]);
    }

    #[rstest]
    #[case("AAPL-USD", true)]
    #[case("BRK.B-USD", true)]
    #[case("", false)]
    #[case("..", false)]
    #[case("AAPL/USD", false)]
    #[case("AAPL-USD?x=1", false)]
    fn test_path_segment(#[case] symbol: &str, #[case] ok: bool) {
        assert_eq!(path_segment(symbol).is_ok(), ok);
    }

    #[tokio::test]
    async fn test_public_request_sends_the_api_version_and_no_credentials() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        let router = Router::new().route(
            "/refdata",
            get(move |RawQuery(query): RawQuery, headers: HeaderMap| {
                record
                    .lock()
                    .unwrap()
                    .push((query, headers.contains_key(HEADER_SIGNATURE)));
                async { fixture_json("refdata") }
            }),
        );
        let client = client(serve(router).await, 0, false);

        let instruments = client.request_instruments().await.unwrap();

        // 17 symbols in the fixture, two of them delisted.
        assert_eq!(instruments.len(), 15);
        assert!(!client.has_credentials());
        assert_eq!(
            *seen.lock().unwrap(),
            vec![(Some("api_version=2026-10-29".to_string()), false)]
        );
    }

    #[tokio::test]
    async fn test_bars_and_funding_are_ascending_and_limited() {
        let queries = Arc::new(Mutex::new(Vec::new()));
        let seen = queries.clone();
        let router = Router::new()
            .route(
                "/candles/AAPL-USD",
                get(move |RawQuery(query): RawQuery| {
                    seen.lock().unwrap().push(query.unwrap());
                    async { fixture_json("candles") }
                }),
            )
            .route(
                "/funding/AAPL-USD",
                get(|| async { fixture_json("funding") }),
            );
        let client = client(serve(router).await, 0, false);
        let aapl = instrument("AAPL-USD");
        let bar_type = BarType::from("AAPL-USD-PERP.QFEX-1-MINUTE-LAST-EXTERNAL");
        let end = UnixNanos::from(1_794_323_100_000_000_000);

        let bars = client
            .request_bars(bar_type, &aapl, None, Some(end), Some(3))
            .await
            .unwrap();
        let rates = client
            .request_funding_rates(&aapl, None, None, Some(4))
            .await
            .unwrap();

        // The fixture holds 5 candles, newest first.
        assert_eq!(bars.len(), 3);
        assert!(
            bars.windows(2)
                .all(|pair| pair[0].ts_event < pair[1].ts_event)
        );
        assert_eq!(bars[2].ts_event, UnixNanos::from(1_794_323_040_000_000_000));
        assert_eq!(rates.len(), 4);
        assert!(
            rates
                .windows(2)
                .all(|pair| pair[0].ts_event < pair[1].ts_event)
        );
        assert_eq!(
            *queries.lock().unwrap(),
            vec![
                "resolution=1MIN&fromISO=2026-11-10T15%3A02%3A00Z&toISO=2026-11-10T15%3A05%3A00Z\
                 &api_version=2026-10-29"
                    .to_string()
            ]
        );

        let unsupported = BarType::from("AAPL-USD-PERP.QFEX-2-MINUTE-LAST-EXTERNAL");
        let error = client
            .request_bars(unsupported, &aapl, None, None, None)
            .await
            .unwrap_err();
        assert!(matches!(error, QfexHttpError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn test_book_snapshot() {
        let router = Router::new().route(
            "/md/orderbook/AAPL-USD",
            get(|| async { fixture_json("orderbook") }),
        );
        let client = client(serve(router).await, 0, false);

        let deltas = client
            .request_book_snapshot(&instrument("AAPL-USD"))
            .await
            .unwrap();

        assert_eq!(deltas.ts_event, UnixNanos::from(1_794_336_612_802_000_000));
        // 82 bid and 1025 ask rows in the fixture; only 20 + 20 have a size. Plus the clear.
        assert_eq!(deltas.deltas.len(), 41);
    }

    #[tokio::test]
    async fn test_authenticated_request_is_signed() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        let router = Router::new().route(
            "/user/trade",
            get(move |RawQuery(query): RawQuery, headers: HeaderMap| {
                let header = |name: &str| headers[name].to_str().unwrap().to_string();
                record.lock().unwrap().push((
                    query.unwrap(),
                    header(HEADER_PUBLIC_KEY),
                    header(HEADER_SIGNATURE),
                    header(HEADER_NONCE),
                    header(HEADER_TIMESTAMP),
                ));
                async { fixture_json("user_trades") }
            }),
        );
        let client = client(serve(router).await, 0, true);
        let query = QfexUserQuery {
            symbol: Some("AAPL-USD".to_string()),
            order_id: Some("abc".to_string()),
            limit: Some(5_000),
            ..Default::default()
        };

        let trades = client.get_user_trades(&query).await.unwrap();

        assert_eq!(trades.data.len(), 2);
        assert_eq!(
            *seen.lock().unwrap(),
            vec![(
                "symbol=AAPL-USD&exchange_order_id=abc&limit=1000&api_version=2026-10-29"
                    .to_string(),
                API_KEY.to_string(),
                SIGNATURE.to_string(),
                NONCE.to_string(),
                "1794336834".to_string(),
            )]
        );
    }

    #[tokio::test]
    async fn test_other_authenticated_endpoints() {
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let seen = bodies.clone();
        let router = Router::new()
            .route(
                "/user/positions",
                get(|| async { fixture_json("user_positions") }),
            )
            .route(
                "/user/historic-orders",
                get(|| async { fixture_json("historic_orders") }),
            )
            .route("/user/fees", get(|| async { fixture_json("user_fees") }))
            .route(
                "/user/leverage",
                post(move |headers: HeaderMap, body: Bytes| {
                    seen.lock().unwrap().push((
                        headers.contains_key(HEADER_SIGNATURE),
                        String::from_utf8(body.to_vec()).unwrap(),
                    ));
                    async { "{}" }
                }),
            );
        let client = client(serve(router).await, 0, true);

        let account = client.get_user_positions().await.unwrap();
        let orders = client
            .get_historic_orders(&QfexUserQuery::default())
            .await
            .unwrap();
        let fees = client.get_fees().await.unwrap();
        client.set_leverage("AAPL-USD", dec!(5)).await.unwrap();

        assert_eq!(account.positions.len(), 2);
        assert_eq!(orders.data.len(), 2);
        assert_eq!(fees.fees.len(), 1);
        assert_eq!(
            *bodies.lock().unwrap(),
            vec![(true, r#"{"symbol":"AAPL-USD","leverage":5.0}"#.to_string())]
        );
    }

    #[tokio::test]
    async fn test_authenticated_request_without_credentials_is_not_sent() {
        let (hits, route) = counted(StatusCode::OK, "{}");
        let router = Router::new()
            .route("/user/positions", route.clone())
            .route("/user/leverage", route);
        let client = client(serve(router).await, 2, false);

        let get_error = client.get_user_positions().await.unwrap_err();
        let post_error = client.set_leverage("AAPL-USD", dec!(2)).await.unwrap_err();

        assert!(matches!(get_error, QfexHttpError::InvalidRequest(_)));
        assert!(matches!(post_error, QfexHttpError::InvalidRequest(_)));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[rstest]
    #[case(
        StatusCode::BAD_REQUEST,
        "problem",
        "venue error Bad Request: unsupported",
        1
    )]
    #[case(StatusCode::NOT_FOUND, "", "HTTP 404", 1)]
    #[case(StatusCode::TOO_MANY_REQUESTS, "", "rate limit exceeded", 3)]
    #[case(StatusCode::BAD_GATEWAY, "problem", "HTTP 502", 3)]
    #[tokio::test]
    async fn test_error_statuses(
        #[case] status: StatusCode,
        #[case] body_fixture: &str,
        #[case] expected: &str,
        #[case] expected_hits: usize,
    ) {
        let body: &'static str = if body_fixture.is_empty() {
            ""
        } else {
            Box::leak(fixture_json(body_fixture).into_boxed_str())
        };
        let (hits, route) = counted(status, body);
        let client = client(
            serve(Router::new().route("/refdata", route)).await,
            2,
            false,
        );

        let error = client.get_refdata().await.unwrap_err();

        assert!(error.to_string().starts_with(expected), "{error}");
        // Only 429 and 5xx are sent again (two retries).
        assert_eq!(hits.load(Ordering::SeqCst), expected_hits);
    }

    #[tokio::test]
    async fn test_unreadable_body_is_a_parse_error() {
        let (hits, route) = counted(StatusCode::OK, "<html>");
        let client = client(
            serve(Router::new().route("/refdata", route)).await,
            2,
            false,
        );

        let error = client.get_refdata().await.unwrap_err();

        assert!(matches!(error, QfexHttpError::Parse(_)));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
