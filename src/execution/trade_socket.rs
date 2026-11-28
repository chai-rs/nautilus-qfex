use nautilus_live::{SocketControl, SocketControlFactory};
use nautilus_model::identifiers::ClientId;
use nautilus_network::SocketStateSink;

use crate::common::consts::QFEX_VENUE;

const TRADE_ENDPOINT: &str = "trade";

#[derive(Debug)]
pub(super) struct StateOnlyTradeSocketControl {
    control: Option<SocketControl>,
}

impl StateOnlyTradeSocketControl {
    pub(super) const fn built_on_first_connect() -> Self {
        Self { control: None }
    }

    pub(super) fn sink(&mut self, client_id: ClientId) -> SocketStateSink {
        self.control
            .get_or_insert_with(|| {
                SocketControlFactory::new(client_id, Some(*QFEX_VENUE)).control(TRADE_ENDPOINT)
            })
            .sink()
    }
}
