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
    common::{consts::QFEX, enums::QfexEnvironment},
    config::{data::QfexDataClientConfig, exec::QfexExecClientConfig},
    factories::{data::QfexDataClientFactory, exec::QfexExecutionClientFactory},
};
use nautilus_testkit::testers::{ExecTester, ExecTesterConfig};
use rust_decimal_macros::dec;

// A stock perpetual: it only trades in its market hours.
const INSTRUMENT_ID: &str = "AAPL-USD-PERP.QFEX";
// AAPL-USD: lot size 0.001, minimum order size 0.01.
const ORDER_QTY: &str = "0.010";
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

    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let (Some(step), None) = (args.next(), args.next()) else {
            return Err(Self::usage());
        };
        Self::ALL
            .into_iter()
            .find(|candidate| (*candidate as u8).to_string() == step)
            .ok_or_else(|| format!("unknown step '{step}'\n{}", Self::usage()))
    }

    fn usage() -> String {
        let steps: String = Self::ALL
            .iter()
            .map(|step| format!("\n  {}  {}", *step as u8, step.what_it_does()))
            .collect();
        format!(
            "usage: cargo run --example exec_tester --features examples -- <step>\n\
             Runs one step of \"The UAT gate\" in docs/STATUS.md on QFEX UAT with \
             QFEX_UAT_API_KEY and QFEX_UAT_API_SECRET, and records it to \
             recordings/uat-step-<step>.jsonl. Stop it with Ctrl-C.{steps}"
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

    fn tester_config(self) -> Result<ExecTesterConfig, Box<dyn std::error::Error>> {
        let instrument_id = InstrumentId::from(INSTRUMENT_ID);
        let limit_buy = matches!(
            self,
            Self::Login | Self::SubmitAndCancel | Self::Modify | Self::DropWithOpenOrder
        );
        let mut config = ExecTesterConfig::builder()
            .instrument_id(instrument_id)
            .client_id(ClientId::new(QFEX))
            .order_qty(Quantity::from(ORDER_QTY))
            .dry_run(self == Self::Login)
            .subscribe_quotes(true)
            .subscribe_trades(false)
            .subscribe_book(false)
            .enable_limit_buys(limit_buy)
            .enable_limit_sells(false)
            .enable_stop_buys(self == Self::StopOrder)
            .enable_stop_sells(false)
            .stop_order_type(OrderType::StopMarket)
            .tob_offset_ticks(100)
            .use_post_only(true)
            .modify_orders_to_maintain_tob_offset(self == Self::Modify)
            .trigger_limit_order_maintenance_once(self == Self::Modify)
            .maybe_open_position_on_start_qty((self == Self::MarketFill).then_some(dec!(0.010)))
            .open_position_on_first_quote(self == Self::MarketFill)
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
    let step = match UatStep::from_args(std::env::args().skip(1)) {
        Ok(step) => step,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    println!(
        "QFEX UAT step {}: {}. Stop with Ctrl-C.",
        step as u8,
        step.what_it_does()
    );
    let (data_config, exec_config) = step.client_configs();
    let tester_config = step.tester_config()?;
    std::fs::create_dir_all(RECORDINGS_DIR)?;

    let instrument_id = InstrumentId::from(INSTRUMENT_ID);
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
    fn no_step_an_unknown_step_or_mainnet_is_refused(#[case] list: &[&str]) {
        let usage = UatStep::from_args(args(list)).unwrap_err();
        assert!(usage.contains("QFEX UAT"), "{usage}");
        assert!(usage.contains("drop the network by hand"), "{usage}");
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
        assert_eq!(UatStep::from_args(args(&[&number.to_string()])), Ok(step));
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
        let login = UatStep::Login.tester_config().unwrap();
        assert!(login.dry_run);

        let submit = UatStep::SubmitAndCancel.tester_config().unwrap();
        assert!(!submit.dry_run && submit.use_post_only);
        assert!(submit.enable_limit_buys && !submit.enable_limit_sells);
        assert!(submit.cancel_orders_on_stop);
        assert!(!submit.modify_orders_to_maintain_tob_offset);

        let modify = UatStep::Modify.tester_config().unwrap();
        assert!(modify.enable_limit_buys && modify.use_post_only);
        assert!(modify.modify_orders_to_maintain_tob_offset);
        assert!(modify.trigger_limit_order_maintenance_once);

        let market = UatStep::MarketFill.tester_config().unwrap();
        assert!(!market.enable_limit_buys && !market.enable_limit_sells);
        assert_eq!(market.open_position_on_start_qty, Some(dec!(0.010)));
        assert!(market.open_position_on_first_quote);
        assert!(market.close_positions_on_stop);

        let stop = UatStep::StopOrder.tester_config().unwrap();
        assert!(!stop.enable_limit_buys && !stop.enable_limit_sells);
        assert!(stop.enable_stop_buys && !stop.enable_stop_sells);
        assert_eq!(stop.stop_order_type, OrderType::StopMarket);
        assert!(stop.cancel_orders_on_stop);

        let drop = UatStep::DropWithOpenOrder.tester_config().unwrap();
        assert!(drop.enable_limit_buys && drop.use_post_only && !drop.dry_run);
    }
}
