//! Flattens a QFEX account: cancels every open order and every stop order, then closes every
//! position with a reduce-only IOC market order.
//!
//! UNVERIFIED: this binary compiles and its plan is unit-tested, but it was never run against
//! the venue (see `docs/STATUS.md`). Run it on UAT first.
//!
//! The plan is printed first and nothing that changes the account is sent before Enter is
//! pressed. Reading the plan needs the key: `GET /user/positions`, then the login on the trade
//! socket and `get_user_orders`.
//!
//! TWAPs are not read and not cancelled: a running TWAP keeps sending orders after this.
//!
//! Environment selection: pass `uat` as the first argv to target UAT; defaults to mainnet.
//!
//! Environment variables (read at run time only):
//! - `QFEX_API_KEY`, `QFEX_API_SECRET` (or the `QFEX_UAT_*` variants).
//!
//! Run with:
//!
//! ```bash
//! cargo run --bin qfex-flatten        # mainnet
//! cargo run --bin qfex-flatten uat    # UAT
//! ```

use std::{
    env,
    io::{BufRead, Write},
    time::Duration,
};

use anyhow::Context;
use nautilus_qfex::{
    common::{
        consts::HTTP_TIMEOUT_SECS,
        credential::QfexCredential,
        enums::{QfexEnvironment, QfexOrderType, QfexSide, QfexStatusKind, QfexTimeInForce},
        urls::qfex_trade_ws_url,
    },
    http::{client::QfexHttpClient, models::QfexPosition},
    signing::QfexSigner,
    websocket::{
        client::QfexWebSocketClient,
        messages::{QfexAddOrder, QfexTradeRequest, QfexWsMessage, QfexWsOrders},
    },
};
use tokio::sync::mpsc::UnboundedReceiver;

/// Longest wait for the login and for one page of open orders; also how long the answers to
/// the plan are printed.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// The default page of `get_user_orders`, which is also the largest the docs mention.
const OPEN_ORDERS_LIMIT: u32 = 1_000;
const MAX_PAGES: u32 = 20;

/// The frames that flatten the account, in sending order: orders go first, because a resting
/// order can block a reduce-only close.
fn plan(open: &QfexWsOrders, positions: &[QfexPosition]) -> Vec<QfexTradeRequest> {
    // A stop order has its own cancel; the venue lists it in `stop_orders` or, by its type,
    // in `orders`.
    let (stops, plain): (Vec<_>, Vec<_>) = open
        .orders
        .iter()
        .partition(|order| order.order_type.is_some_and(QfexOrderType::is_stop));

    let mut requests = Vec::new();
    if !plain.is_empty() {
        // Without a symbol: every active order of the account.
        requests.push(QfexTradeRequest::CancelAllOrders { symbol: None });
    }
    for stop in stops.into_iter().chain(&open.stop_orders) {
        requests.push(QfexTradeRequest::CancelStopOrder {
            stop_order_id: stop.order_id.clone(),
        });
    }
    for position in positions.iter().filter(|p| !p.position.is_zero()) {
        requests.push(QfexTradeRequest::AddOrder(QfexAddOrder {
            symbol: position.symbol,
            side: if position.position.is_sign_positive() {
                QfexSide::Sell
            } else {
                QfexSide::Buy
            },
            order_type: QfexOrderType::Market,
            order_time_in_force: QfexTimeInForce::Ioc,
            quantity: position.position.abs(),
            price: None,
            // Empty: the venue generates one (Add Order page of the docs).
            client_order_id: String::new(),
            reduce_only: true,
        }));
    }
    requests
}

/// What one frame of the venue says about the plan: `Ok` for progress, `Err` for a refusal,
/// `None` for a frame that is not an answer.
fn outcome(message: &QfexWsMessage) -> Option<Result<String, String>> {
    match message {
        QfexWsMessage::Order(order) => {
            let line = format!(
                "order {} {} {}",
                order.symbol,
                order.order_id,
                order.status.as_ref()
            );
            Some(match order.status.kind() {
                QfexStatusKind::Rejected
                | QfexStatusKind::CancelRejected
                | QfexStatusKind::ModifyRejected => Err(line),
                _ => Ok(line),
            })
        }
        QfexWsMessage::Fill(fill) => Some(Ok(format!(
            "fill {} {} {} @ {}",
            fill.symbol,
            fill.side.as_ref(),
            fill.quantity,
            fill.price
        ))),
        QfexWsMessage::Error(error) => Some(Err(format!("error: {}", error.message))),
        _ => None,
    }
}

/// Waits for the frame `pick` takes. An error frame, a reconnect or a silent socket ends the
/// wait: the answer to a request cannot be told from the answer to an older one after that.
async fn wait_for<T>(
    rx: &mut UnboundedReceiver<QfexWsMessage>,
    what: &str,
    pick: impl Fn(QfexWsMessage) -> Option<T>,
) -> anyhow::Result<T> {
    let answer = async {
        loop {
            match rx.recv().await {
                Some(QfexWsMessage::Error(error)) => {
                    anyhow::bail!("QFEX refused {what}: {}", error.message)
                }
                Some(QfexWsMessage::Reconnected) => {
                    anyhow::bail!("the trade socket reconnected during {what}; run again")
                }
                Some(message) => {
                    if let Some(found) = pick(message) {
                        return Ok(found);
                    }
                }
                None => anyhow::bail!("the trade socket closed during {what}"),
            }
        }
    };
    tokio::time::timeout(ANSWER_TIMEOUT, answer)
        .await
        .with_context(|| format!("QFEX did not answer {what}"))?
}

/// Open orders and open stop orders: pages of `get_user_orders` until a short page.
async fn open_orders(
    ws: &QfexWebSocketClient,
    rx: &mut UnboundedReceiver<QfexWsMessage>,
) -> anyhow::Result<QfexWsOrders> {
    let mut open = QfexWsOrders::default();
    for page in 0..MAX_PAGES {
        ws.send(&QfexTradeRequest::GetUserOrders {
            limit: OPEN_ORDERS_LIMIT,
            offset: page * OPEN_ORDERS_LIMIT,
        })
        .await?;
        let batch = wait_for(rx, "get_user_orders", |message| match message {
            QfexWsMessage::Orders(orders) => Some(orders),
            _ => None,
        })
        .await?;
        let rows = batch.orders.len() + batch.stop_orders.len();
        open.orders.extend(batch.orders);
        open.stop_orders.extend(batch.stop_orders);
        if rows < OPEN_ORDERS_LIMIT as usize {
            return Ok(open);
        }
    }
    // Stop orders are cancelled one by one, so a cut list would leave some open.
    anyhow::bail!("more than {} open orders", MAX_PAGES * OPEN_ORDERS_LIMIT)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    nautilus_common::logging::ensure_logging_initialized();

    let environment = if env::args().nth(1).is_some_and(|arg| arg == "uat") {
        QfexEnvironment::Uat
    } else {
        QfexEnvironment::Mainnet
    };
    let credential =
        QfexCredential::resolve(None, None, environment)?.context("no QFEX credentials in env")?;
    let signer = QfexSigner::new(credential);

    let http = QfexHttpClient::new(
        environment,
        None,
        None,
        HTTP_TIMEOUT_SECS,
        None,
        Some(signer.clone()),
    )?;
    let positions = http.get_user_positions().await?.positions;

    let mut ws = QfexWebSocketClient::new_trade(qfex_trade_ws_url(environment), signer, false)?;
    let mut rx = ws.connect().await?;
    wait_for(&mut rx, "the login", |message| {
        matches!(message, QfexWsMessage::Authenticated).then_some(())
    })
    .await?;
    let open = open_orders(&ws, &mut rx).await?;

    let requests = plan(&open, &positions);
    println!();
    println!("QFEX flatten");
    println!("  environment : {environment:?}");
    println!("  open orders : {}", open.orders.len());
    println!("  stop orders : {}", open.stop_orders.len());
    for position in positions.iter().filter(|p| !p.position.is_zero()) {
        println!("  position    : {} {}", position.symbol, position.position);
    }
    if requests.is_empty() {
        println!("Nothing to do.");
        ws.disconnect().await;
        return Ok(());
    }
    println!("Frames to send, in this order:");
    for request in &requests {
        println!("  {}", request.to_frame());
    }
    print!("\nProceed? [Enter to continue, Ctrl+C to abort] ");
    std::io::stdout().flush()?;
    std::io::stdin().lock().read_line(&mut String::new())?;

    for request in &requests {
        ws.send(request).await?;
    }

    // The venue answers on the socket, without a request id: print what comes for a while.
    let mut failed = 0;
    let wait = tokio::time::sleep(ANSWER_TIMEOUT);
    tokio::pin!(wait);
    loop {
        tokio::select! {
            () = &mut wait => break,
            message = rx.recv() => match message.as_ref().map(outcome) {
                Some(Some(Ok(line))) => println!("{line}"),
                Some(Some(Err(line))) => {
                    println!("FAILED {line}");
                    failed += 1;
                }
                Some(None) => {}
                None => break,
            },
        }
    }
    ws.disconnect().await;

    // shortcut: one IOC order per position, so a thin book leaves a rest. Run again until
    // it prints "Nothing to do"; upgrade to a fill-checking loop if that is too manual.
    anyhow::ensure!(
        failed == 0,
        "{failed} refusal(s), check the account and run again"
    );
    println!("Sent. Run again to confirm the account is flat.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use serde_json::{Value, json};
    use ustr::Ustr;

    use super::*;

    fn position(symbol: &str, position: Decimal) -> QfexPosition {
        QfexPosition {
            symbol: Ustr::from(symbol),
            position,
            average_price: dec!(100),
        }
    }

    fn frames(requests: &[QfexTradeRequest]) -> Vec<Value> {
        requests
            .iter()
            .map(|request| serde_json::from_str(&request.to_frame()).unwrap())
            .collect()
    }

    fn close(symbol: &str, side: &str, quantity: f64) -> Value {
        json!({"type": "add_order", "params": {
            "symbol": symbol, "side": side, "order_type": "MARKET",
            "order_time_in_force": "IOC", "quantity": quantity, "client_order_id": "",
            "reduce_only": true,
        }})
    }

    #[rstest]
    fn test_plan_cancels_first_then_closes_every_position() {
        let open: QfexWsOrders = serde_json::from_value(json!({
            "orders": [
                {"order_id": "o-1", "symbol": "AAPL-USD", "status": "ACK", "type": "LIMIT"},
                {"order_id": "s-1", "symbol": "AAPL-USD", "status": "ACK", "type": "STOP_LOSS"},
                {"order_id": "o-2", "symbol": "GOOG-USD", "status": "ACK", "type": "ALO"},
            ],
            "stop_orders": [
                {"order_id": "s-2", "symbol": "GOOG-USD", "status": "ACK", "type": "TAKE_PROFIT"},
            ],
        }))
        .unwrap();
        let positions = [
            position("AAPL-USD", dec!(2)),
            position("GOOG-USD", dec!(-1.5)),
            position("TSLA-USD", dec!(0)),
        ];

        assert_eq!(
            frames(&plan(&open, &positions)),
            vec![
                // One mass cancel for the two plain orders, whatever their symbols.
                json!({"type": "cancel_all_orders", "params": {}}),
                json!({"type": "cancel_stop_order", "params": {"stop_order_id": "s-1"}}),
                json!({"type": "cancel_stop_order", "params": {"stop_order_id": "s-2"}}),
                // A long is sold, a short is bought, a flat position is left alone.
                close("AAPL-USD", "SELL", 2.0),
                close("GOOG-USD", "BUY", 1.5),
            ]
        );
    }

    #[rstest]
    fn test_plan_sends_only_what_is_needed() {
        let none = QfexWsOrders::default();
        assert!(plan(&none, &[]).is_empty());
        assert!(plan(&none, &[position("AAPL-USD", dec!(0))]).is_empty());

        // Stop orders alone: no mass cancel.
        let stops: QfexWsOrders = serde_json::from_value(json!({
            "stop_orders": [{"order_id": "s-1", "status": "ACK"}],
        }))
        .unwrap();
        assert_eq!(
            frames(&plan(&stops, &[])),
            vec![json!({"type": "cancel_stop_order", "params": {"stop_order_id": "s-1"}})]
        );

        // The recorded `all_orders_response` (one plain order) and no position.
        let line = include_str!("../tests/fixtures/ws/trade.jsonl")
            .lines()
            .nth(9)
            .unwrap();
        let QfexWsMessage::Orders(recorded) = QfexWsMessage::parse(line).unwrap() else {
            panic!("line 9 is not an all_orders_response");
        };
        assert_eq!(
            frames(&plan(&recorded, &[])),
            vec![json!({"type": "cancel_all_orders", "params": {}})]
        );
    }

    #[rstest]
    #[case(json!({"order_response": {"order_id": "o-1", "symbol": "AAPL-USD", "status": "CANCELLED"}}),
        Some(Ok("order AAPL-USD o-1 CANCELLED")))]
    #[case(json!({"order_response": {"order_id": "o-9", "symbol": "AAPL-USD", "status": "IOC_CANCELLED"}}),
        Some(Ok("order AAPL-USD o-9 IOC_CANCELLED")))]
    #[case(json!({"order_response": {"symbol": "AAPL-USD", "status": "REJECTED_MARKET_CLOSED"}}),
        Some(Err("order AAPL-USD  REJECTED_MARKET_CLOSED")))]
    #[case(json!({"order_response": {"order_id": "s-1", "status": "NO_SUCH_ORDER"}}),
        Some(Err("order  s-1 NO_SUCH_ORDER")))]
    #[case(json!({"err": {"error_code": "RateLimited", "message": "too fast"}}),
        Some(Err("error: too fast")))]
    #[case(json!({"balance_response": {"available_balance": 1}}), None)]
    fn test_outcome_tells_a_refusal_from_progress(
        #[case] frame: Value,
        #[case] expected: Option<Result<&str, &str>>,
    ) {
        let message = QfexWsMessage::parse(&frame.to_string()).unwrap();
        let outcome = outcome(&message);
        let outcome = outcome
            .as_ref()
            .map(|result| result.as_deref().map_err(String::as_str));
        assert_eq!(outcome, expected);
    }
}
