use std::path::PathBuf;

use log::LevelFilter;
use nautilus_common::{enums::Environment, logging::logger::LoggerConfig};
use nautilus_live::{config::LiveExecutionEngineConfig, node::LiveNode};
use nautilus_model::{
    enums::OrderType,
    identifiers::{ClientId, InstrumentId, StrategyId, TraderId},
    types::Quantity,
};
use nautilus_qfex::{
    common::{consts::QFEX, enums::QfexEnvironment, parse::instrument_id_of},
    config::{data::QfexDataClientConfig, exec::QfexExecClientConfig},
    factories::{data::QfexDataClientFactory, exec::QfexExecutionClientFactory},
};
use nautilus_testkit::testers::{ExecTester, ExecTesterConfig};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// A stock perpetual: it only trades in its market hours.
const DEFAULT_INSTRUMENT_ID: &str = "AAPL-USD-PERP.QFEX";
// AAPL-USD: lot size 0.001, minimum order size 0.01.
const DEFAULT_ORDER_QTY: Decimal = dec!(0.010);
const RECORDINGS_DIR: &str = "recordings";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UatStep {
    Login = 2,
    SubmitAndCancel = 3,
    Modify = 4,
    MarketFill = 5,
    StopOrder = 6,
    DropWithOpenOrder = 7,
}

impl UatStep {
    const ALL: [Self; 6] = [
        Self::Login,
        Self::SubmitAndCancel,
        Self::Modify,
        Self::MarketFill,
        Self::StopOrder,
        Self::DropWithOpenOrder,
    ];

    fn usage() -> String {
        let steps: String = Self::ALL
            .iter()
            .map(|step| format!("\n  {}  {}", *step as u8, step.what_it_does()))
            .collect();
        format!(
            "usage: cargo run --example exec_tester --features examples -- <step> [SYMBOL QTY]\n\
             Runs one step of \"The UAT gate\" in docs/STATUS.md on QFEX UAT with \
             QFEX_UAT_API_KEY and QFEX_UAT_API_SECRET, and records it to \
             recordings/uat-step-<step>.jsonl. Stop it with Ctrl-C. SYMBOL and QTY name \
             the instrument and the order size, for example GOLD-USD 0.001; without them \
             the tester trades {DEFAULT_ORDER_QTY} AAPL-USD.{steps}"
        )
    }

    const fn what_it_does(self) -> &'static str {
        match self {
            Self::Login => "connect, log in and reconcile; no order (dry run)",
            Self::SubmitAndCancel => {
                "one post-only limit buy far from the market, cancelled on Ctrl-C"
            }
            Self::Modify => "as 3, then one modify of the resting order",
            Self::MarketFill => "one small market buy, closed on Ctrl-C",
            Self::StopOrder => "one stop-market buy, cancelled on Ctrl-C",
            Self::DropWithOpenOrder => {
                "one resting post-only limit buy with cancel on disconnect; \
                 drop the network by hand for more than 20 s, then bring it back"
            }
        }
    }

    fn client_configs(self) -> (QfexDataClientConfig, QfexExecClientConfig) {
        let data = QfexDataClientConfig {
            environment: QfexEnvironment::Uat,
            ..Default::default()
        };
        let exec = QfexExecClientConfig {
            environment: QfexEnvironment::Uat,
            cancel_on_disconnect: self == Self::DropWithOpenOrder,
            record_path: Some(
                PathBuf::from(RECORDINGS_DIR).join(format!("uat-step-{}.jsonl", self as u8)),
            ),
            ..Default::default()
        };
        (data, exec)
    }
}

#[derive(Debug)]
struct UatRun {
    step: UatStep,
    instrument_id: InstrumentId,
    order_qty: Decimal,
}

impl UatRun {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let args: Vec<String> = args.collect();
        let refuse = |why: String| format!("{why}\n{}", UatStep::usage());
        let (step, instrument) = match args.as_slice() {
            [step] => (step, None),
            [step, symbol, qty] => (step, Some((symbol, qty))),
            _ => return Err(UatStep::usage()),
        };
        let step = UatStep::ALL
            .into_iter()
            .find(|candidate| (*candidate as u8).to_string() == *step)
            .ok_or_else(|| refuse(format!("unknown step '{step}'")))?;
        let Some((symbol, qty)) = instrument else {
            return Ok(Self {
                step,
                instrument_id: InstrumentId::from(DEFAULT_INSTRUMENT_ID),
                order_qty: DEFAULT_ORDER_QTY,
            });
        };
        let venue_symbol = symbol.contains('-')
            && symbol
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-');
        if !venue_symbol {
            return Err(refuse(format!(
                "'{symbol}' is not a QFEX symbol such as GOLD-USD"
            )));
        }
        let instrument_id = instrument_id_of(symbol).map_err(|e| refuse(e.to_string()))?;
        let order_qty = qty
            .parse::<Decimal>()
            .ok()
            .filter(|qty| qty.is_sign_positive() && !qty.is_zero())
            .ok_or_else(|| refuse(format!("'{qty}' is not a positive quantity")))?;
        Ok(Self {
            step,
            instrument_id,
            order_qty,
        })
    }

    fn tester_config(&self) -> Result<ExecTesterConfig, Box<dyn std::error::Error>> {
        let Self {
            step,
            instrument_id,
            order_qty,
        } = *self;
        let limit_buy = matches!(
            step,
            UatStep::Login
                | UatStep::SubmitAndCancel
                | UatStep::Modify
                | UatStep::DropWithOpenOrder
        );
        let mut config = ExecTesterConfig::builder()
            .instrument_id(instrument_id)
            .client_id(ClientId::new(QFEX))
            .order_qty(Quantity::from(order_qty.to_string().as_str()))
            .dry_run(step == UatStep::Login)
            .subscribe_quotes(true)
            .subscribe_trades(false)
            .subscribe_book(false)
            .enable_limit_buys(limit_buy)
            .enable_limit_sells(false)
            .enable_stop_buys(step == UatStep::StopOrder)
            .enable_stop_sells(false)
            .stop_order_type(OrderType::StopMarket)
            .tob_offset_ticks(100)
            .use_post_only(true)
            .modify_orders_to_maintain_tob_offset(step == UatStep::Modify)
            .trigger_limit_order_maintenance_once(step == UatStep::Modify)
            .maybe_open_position_on_start_qty((step == UatStep::MarketFill).then_some(order_qty))
            .open_position_on_first_quote(step == UatStep::MarketFill)
            .cancel_orders_on_stop(true)
            .close_positions_on_stop(true)
            .log_data(false)
            .build()?;
        config.base.strategy_id = Some(StrategyId::from("EXEC_TESTER-001"));
        config.base.external_order_instrument_ids = Some(vec![instrument_id]);
        Ok(config)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run = match UatRun::from_args(std::env::args().skip(1)) {
        Ok(run) => run,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    println!(
        "QFEX UAT step {}: {}, {} {}. Stop with Ctrl-C.",
        run.step as u8,
        run.step.what_it_does(),
        run.order_qty,
        run.instrument_id
    );
    let (data_config, exec_config) = run.step.client_configs();
    let tester_config = run.tester_config()?;
    let instrument_id = run.instrument_id;
    std::fs::create_dir_all(RECORDINGS_DIR)?;

    let log_config = LoggerConfig {
        stdout_level: LevelFilter::Info,
        ..Default::default()
    };
    let exec_engine_config = LiveExecutionEngineConfig {
        reconciliation_lookback_mins: Some(2 * 24 * 60),
        reconciliation_instrument_ids: Some(vec![instrument_id.to_string()]),
        open_check_interval_secs: Some(10.0),
        position_check_interval_secs: Some(30.0),
        ..Default::default()
    };

    let mut node = LiveNode::builder(TraderId::from("TESTER-001"), Environment::Live)?
        .with_name("QFEX-EXEC-TESTER-001".to_string())
        .with_logging(log_config)
        .with_exec_engine_config(exec_engine_config)
        // The tester quotes off the top of book, so it needs the data client too.
        .add_data_client(
            Some(QFEX.to_string()),
            Box::new(QfexDataClientFactory::new()),
            Box::new(data_config),
        )?
        .add_exec_client(
            Some(QFEX.to_string()),
            Box::new(QfexExecutionClientFactory),
            Box::new(exec_config),
        )?
        .with_reconciliation(true)
        .with_delay_post_stop_secs(5)
        .build()?;

    node.add_strategy(ExecTester::new(tester_config))?;
    node.run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter().map(ToString::to_string)
    }

    #[rstest]
    #[case(&[])]
    #[case(&["1"])]
    #[case(&["8"])]
    #[case(&["three"])]
    #[case(&["mainnet"])]
    #[case(&["3", "mainnet"])]
    #[case(&["3", "mainnet", "1"])]
    #[case(&["3", "GOLD-USD"])]
    #[case(&["3", "GOLD-USD", "0"])]
    #[case(&["3", "GOLD-USD", "-0.001"])]
    #[case(&["3", "GOLD-USD", "lots"])]
    #[case(&["3", "GOLD-USD", "0.001", "extra"])]
    fn no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused(#[case] list: &[&str]) {
        let usage = UatRun::from_args(args(list)).unwrap_err();
        assert!(usage.contains("QFEX UAT"), "{usage}");
        assert!(usage.contains("drop the network by hand"), "{usage}");
    }

    #[test]
    fn the_instrument_defaults_to_aapl_and_can_be_named_with_its_quantity() {
        let default = UatRun::from_args(args(&["3"])).unwrap();
        assert_eq!(
            default.instrument_id,
            InstrumentId::from("AAPL-USD-PERP.QFEX")
        );
        assert_eq!(default.order_qty, dec!(0.010));

        let gold = UatRun::from_args(args(&["5", "GOLD-USD", "0.001"])).unwrap();
        assert_eq!(gold.step, UatStep::MarketFill);
        assert_eq!(gold.instrument_id, InstrumentId::from("GOLD-USD-PERP.QFEX"));
        let config = gold.tester_config().unwrap();
        assert_eq!(config.instrument_id, gold.instrument_id);
        assert_eq!(config.order_qty, Quantity::from("0.001"));
        assert_eq!(config.open_position_on_start_qty, Some(dec!(0.001)));
        assert_eq!(
            config.base.external_order_instrument_ids,
            Some(vec![gold.instrument_id])
        );
    }

    #[rstest]
    #[case(2, UatStep::Login)]
    #[case(3, UatStep::SubmitAndCancel)]
    #[case(4, UatStep::Modify)]
    #[case(5, UatStep::MarketFill)]
    #[case(6, UatStep::StopOrder)]
    #[case(7, UatStep::DropWithOpenOrder)]
    fn every_step_runs_on_uat_and_records_to_its_own_file(
        #[case] number: u8,
        #[case] step: UatStep,
    ) {
        let run = UatRun::from_args(args(&[&number.to_string()])).unwrap();
        assert_eq!(run.step, step);
        let (data, exec) = step.client_configs();
        assert_eq!(data.environment, QfexEnvironment::Uat);
        assert_eq!(exec.environment, QfexEnvironment::Uat);
        assert_eq!(
            exec.record_path,
            Some(PathBuf::from(format!("recordings/uat-step-{number}.jsonl")))
        );
        assert_eq!(
            exec.cancel_on_disconnect,
            step == UatStep::DropWithOpenOrder
        );
        assert!(exec.api_key.is_none() && exec.api_secret.is_none());
    }

    #[test]
    fn each_step_drives_the_tester_the_way_the_gate_needs() {
        let config = |step: &str| {
            let run = UatRun::from_args(args(&[step])).unwrap();
            run.tester_config().unwrap()
        };
        let login = config("2");
        assert!(login.dry_run);

        let submit = config("3");
        assert!(!submit.dry_run && submit.use_post_only);
        assert!(submit.enable_limit_buys && !submit.enable_limit_sells);
        assert!(submit.cancel_orders_on_stop);
        assert!(!submit.modify_orders_to_maintain_tob_offset);

        let modify = config("4");
        assert!(modify.enable_limit_buys && modify.use_post_only);
        assert!(modify.modify_orders_to_maintain_tob_offset);
        assert!(modify.trigger_limit_order_maintenance_once);

        let market = config("5");
        assert!(!market.enable_limit_buys && !market.enable_limit_sells);
        assert_eq!(market.open_position_on_start_qty, Some(dec!(0.010)));
        assert!(market.open_position_on_first_quote);
        assert!(market.close_positions_on_stop);

        let stop = config("6");
        assert!(!stop.enable_limit_buys && !stop.enable_limit_sells);
        assert!(stop.enable_stop_buys && !stop.enable_stop_sells);
        assert_eq!(stop.stop_order_type, OrderType::StopMarket);
        assert!(stop.cancel_orders_on_stop);

        let drop = config("7");
        assert!(drop.enable_limit_buys && drop.use_post_only && !drop.dry_run);
    }
}
