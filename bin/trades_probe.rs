//! Prints the public trades of one QFEX symbol as they happen, without a `LiveNode`.
//!
//! Run with: `cargo run --bin qfex-trades-probe -- [INSTRUMENT_ID] [LIMIT]`
//!
//! Read-only and public: the mainnet market data socket with one `trade` subscription. No
//! REST request, no account, no key. Stops after `LIMIT` trades (default 20) or on Ctrl+C;
//! QFEX has no public trade history, so a quiet symbol prints nothing until it trades.

use nautilus_model::identifiers::InstrumentId;
use nautilus_qfex::{
    common::{
        enums::{QfexEnvironment, QfexMdsChannel},
        parse::venue_symbol_of,
        urls::qfex_mds_ws_url,
    },
    websocket::{
        client::QfexWebSocketClient,
        messages::{QfexWsMessage, QfexWsTrade},
    },
};
use ustr::Ustr;

const DEFAULT_INSTRUMENT_ID: &str = "US100-USD-PERP.QFEX";
const DEFAULT_LIMIT: usize = 20;

/// The venue symbol and the number of trades to print, from the arguments after the program
/// name.
fn parse_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<(Ustr, usize)> {
    let instrument_id = args.next();
    let instrument_id: InstrumentId = instrument_id
        .as_deref()
        .unwrap_or(DEFAULT_INSTRUMENT_ID)
        .parse()?;
    let limit = args
        .next()
        .map_or(Ok(DEFAULT_LIMIT), |limit| limit.parse())?;
    Ok((Ustr::from(venue_symbol_of(&instrument_id)?), limit))
}

/// One printed line: the venue's own fields, so no instrument (and no REST request) is needed.
fn trade_line(trade: &QfexWsTrade) -> String {
    let side = trade.side.as_ref().map_or("-", |side| side.as_ref());
    format!(
        "{} {} {side} {} @ {} {}",
        trade.time, trade.symbol, trade.size, trade.price, trade.trade_id
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (symbol, limit) = parse_args(std::env::args().skip(1))?;

    let url = qfex_mds_ws_url(QfexEnvironment::default()).to_string();
    let mut client = QfexWebSocketClient::new_mds(url);
    client.subscribe(QfexMdsChannel::Trade, symbol).await?;
    let mut rx = client.connect().await?;

    println!("trades of {symbol}, up to {limit}, Ctrl+C to stop");
    let mut printed = 0;
    while printed < limit {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            message = rx.recv() => match message {
                // The socket sends every subscribed symbol; one is subscribed here.
                Some(QfexWsMessage::Trade(trade)) if trade.symbol == symbol => {
                    println!("{}", trade_line(&trade));
                    printed += 1;
                }
                Some(QfexWsMessage::Error(error)) => anyhow::bail!("QFEX: {}", error.message),
                Some(_) => {}
                None => anyhow::bail!("the market data socket closed"),
            },
        }
    }
    client.disconnect().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn args(args: &[&str]) -> anyhow::Result<(Ustr, usize)> {
        parse_args(args.iter().map(ToString::to_string))
    }

    #[rstest]
    fn test_arguments_give_the_venue_symbol_and_the_limit() {
        assert_eq!(args(&[]).unwrap(), (Ustr::from("US100-USD"), 20));
        assert_eq!(
            args(&["AAPL-USD-PERP.QFEX", "3"]).unwrap(),
            (Ustr::from("AAPL-USD"), 3)
        );
        // Not a QFEX perpetual, not an instrument id, not a number.
        assert!(args(&["AAPL-USD-PERP.RISEX"]).is_err());
        assert!(args(&["AAPL-USD"]).is_err());
        assert!(args(&["AAPL-USD-PERP.QFEX", "many"]).is_err());
    }

    // The frame is line 16 of `tests/fixtures/ws/mds.jsonl`, recorded on 10 Nov 2026.
    #[rstest]
    fn test_trade_line_of_a_recorded_frame() {
        let frame = include_str!("../tests/fixtures/ws/mds.jsonl")
            .lines()
            .nth(16)
            .unwrap();
        let QfexWsMessage::Trade(trade) = QfexWsMessage::parse(frame).unwrap() else {
            panic!("line 16 is not a trade frame");
        };
        assert_eq!(
            trade_line(&trade),
            "2026-11-10T18:54:22.356206457Z US100-USD SELL 0.0075 @ 27587 \
             7223a48a-197b-4797-8b92-8aeff7fd893e"
        );
    }
}
