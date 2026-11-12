//! Routes the order, fill and error frames of the trade socket to the orders they are about.
//!
//! Orders sent by this client are tracked and get order events; anything else (orders of
//! another session, of the web UI) goes to the engine as a report.

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Context;
use nautilus_core::UnixNanos;
use nautilus_live::ExecutionEventEmitter;
use nautilus_model::{
    enums::{LiquiditySide, OrderStatus, TriggerType},
    identifiers::{AccountId, ClientOrderId, TradeId, VenueOrderId},
    instruments::{Instrument, InstrumentAny},
    orders::{Order, OrderAny},
    reports::{FillReport, OrderStatusReport},
    types::Money,
};
use parking_lot::Mutex;
use rust_decimal::Decimal;
use ustr::Ustr;

use super::order::is_stop;
use crate::{
    common::{
        enums::{QfexOrderType, QfexStatusKind, QfexTimeInForce},
        parse::{price_from_decimal, quantity_from_decimal, unix_time_to_nanos, venue_symbol_of},
    },
    http::parse::liquidity_side_of,
    websocket::messages::{QfexWsError, QfexWsFill, QfexWsOrder},
};

const MAX_TRACKED_ORDERS: usize = 4096;
const MAX_SEEN_TRADES: usize = 10_000;

fn checked<T, E: std::fmt::Display>(result: Result<T, E>) -> anyhow::Result<T> {
    result.map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Debug)]
struct Tracked {
    order: OrderAny,
    venue_order_id: Option<VenueOrderId>,
    accepted: bool,
    closed: bool,
}

#[derive(Debug, Default)]
struct State {
    orders: HashMap<ClientOrderId, Tracked>,
    /// Every venue id an order ever had: a modify gives a new one, late rows use the old one.
    by_venue: HashMap<VenueOrderId, ClientOrderId>,
    /// Modifies waiting for their answer, oldest first.
    modifies: VecDeque<ClientOrderId>,
    seen: HashSet<TradeId>,
    seen_queue: VecDeque<TradeId>,
}

impl State {
    /// Returns `false` if the trade was seen before.
    fn mark_seen(&mut self, trade_id: TradeId) -> bool {
        if !self.seen.insert(trade_id) {
            return false;
        }
        self.seen_queue.push_back(trade_id);
        if self.seen_queue.len() > MAX_SEEN_TRADES
            && let Some(oldest) = self.seen_queue.pop_front()
        {
            self.seen.remove(&oldest);
        }
        true
    }

    fn key_of(&self, client_order_id: &str, order_id: &str) -> Option<ClientOrderId> {
        let tracked = |id: &str| {
            ClientOrderId::new_checked(id)
                .ok()
                .filter(|id| self.orders.contains_key(id))
        };
        tracked(client_order_id)
            .or_else(|| {
                let id = VenueOrderId::new_checked(order_id).ok()?;
                self.by_venue.get(&id).copied()
            })
            // A cancel by client order id carries it in `order_id`.
            .or_else(|| tracked(order_id))
    }
}

/// Order state of one trade-socket session.
#[derive(Debug)]
pub struct OrderDispatch {
    account_id: AccountId,
    emitter: ExecutionEventEmitter,
    /// By venue symbol.
    instruments: HashMap<Ustr, InstrumentAny>,
    state: Mutex<State>,
}

impl OrderDispatch {
    #[must_use]
    pub fn new(
        account_id: AccountId,
        emitter: ExecutionEventEmitter,
        instruments: HashMap<Ustr, InstrumentAny>,
    ) -> Self {
        Self {
            account_id,
            emitter,
            instruments,
            state: Mutex::default(),
        }
    }

    /// Starts to route the frames of `order` as order events. Does nothing the second time.
    pub fn track(&self, order: &OrderAny) {
        let mut state = self.state.lock();
        let State {
            orders, by_venue, ..
        } = &mut *state;
        if orders.len() >= MAX_TRACKED_ORDERS {
            // shortcut: closed orders stay tracked until the map is full, then all of them go;
            // a fill that arrives later still reaches the engine, as a FillReport.
            orders.retain(|_, tracked| !tracked.closed);
            by_venue.retain(|_, key| orders.contains_key(key));
        }
        let key = order.client_order_id();
        if let Some(venue_order_id) = order.venue_order_id() {
            by_venue.insert(venue_order_id, key);
        }
        orders.entry(key).or_insert_with(|| Tracked {
            order: order.clone(),
            venue_order_id: order.venue_order_id(),
            // An order the engine already has on the venue must not be accepted again.
            accepted: order.venue_order_id().is_some(),
            closed: false,
        });
    }

    /// Records a modify request that waits for its answer.
    pub fn begin_modify(&self, key: ClientOrderId) {
        self.state.lock().modifies.push_back(key);
    }

    /// Takes a modify back that was never sent.
    pub fn end_modify(&self, key: &ClientOrderId) {
        self.state.lock().modifies.retain(|id| id != key);
    }

    #[must_use]
    pub fn instrument(&self, symbol: &Ustr) -> Option<&InstrumentAny> {
        self.instruments.get(symbol)
    }

    /// Forgets the modifies in flight: after a reconnect their answers will not come.
    pub fn clear_modifies(&self) {
        self.state.lock().modifies.clear();
    }

    /// Stops routing: the engine learned from a report that the order is closed.
    pub fn close(&self, venue_order_id: &VenueOrderId) {
        let mut state = self.state.lock();
        if let Some(key) = state.by_venue.get(venue_order_id).copied()
            && let Some(tracked) = state.orders.get_mut(&key)
        {
            tracked.closed = true;
        }
    }

    #[must_use]
    pub fn venue_order_id(&self, key: &ClientOrderId) -> Option<VenueOrderId> {
        self.state.lock().orders.get(key)?.venue_order_id
    }

    #[must_use]
    pub fn client_order_id(&self, venue_order_id: &VenueOrderId) -> Option<ClientOrderId> {
        self.state.lock().by_venue.get(venue_order_id).copied()
    }

    /// Returns `false` if the trade was reported before; marks it as reported.
    pub fn is_new_trade(&self, trade_id: TradeId) -> bool {
        self.state.lock().mark_seen(trade_id)
    }

    fn accept(&self, tracked: &mut Tracked, ts_event: UnixNanos) {
        if !tracked.accepted
            && !tracked.closed
            && let Some(venue_order_id) = tracked.venue_order_id
        {
            tracked.accepted = true;
            self.emitter
                .emit_order_accepted(&tracked.order, venue_order_id, ts_event);
        }
    }

    fn updated(&self, tracked: &Tracked, row: &QfexWsOrder, ts_event: UnixNanos) {
        let order = &tracked.order;
        let stop = is_stop(order.order_type());
        // The tracked copy is as old as the submit: only its precisions are used.
        let old_price = if stop {
            order.trigger_price()
        } else {
            order.price()
        };
        let (Some(venue_order_id), Some(quantity), Some(price), Some(old_price)) =
            (tracked.venue_order_id, row.quantity, row.price, old_price)
        else {
            log::warn!(
                "QFEX MODIFIED row of {} without id, quantity or price",
                order.client_order_id()
            );
            return;
        };
        match (
            quantity_from_decimal(quantity, order.quantity().precision),
            price_from_decimal(price, old_price.precision),
        ) {
            (Ok(quantity), Ok(price)) => self.emitter.emit_order_updated(
                order,
                venue_order_id,
                quantity,
                (!stop).then_some(price),
                stop.then_some(price),
                None,
                ts_event,
            ),
            (quantity, price) => log::warn!(
                "QFEX MODIFIED row of {} unreadable: {quantity:?} {price:?}",
                order.client_order_id()
            ),
        }
    }

    /// Handles one `order_response`.
    pub fn handle_order(&self, row: &QfexWsOrder, ts_init: UnixNanos) {
        let kind = row.status.kind();
        let reason = row.status.as_ref();
        let ts_event = row
            .update_time
            .and_then(|time| unix_time_to_nanos(time).ok())
            .unwrap_or(ts_init);
        let row_id = VenueOrderId::new_checked(&row.order_id).ok();

        let mut state = self.state.lock();
        let mut key = state.key_of(&row.client_order_id, &row.order_id);
        if key.is_none()
            && matches!(
                kind,
                QfexStatusKind::Modified | QfexStatusKind::ModifyRejected
            )
        {
            // shortcut: the docs show a MODIFIED row with a new order id and no client order
            // id, and requests carry no id of their own, so such a row is given to the oldest
            // modify in flight. Wrong if the venue answers out of order or another session
            // modifies at the same moment; match on a request id once the venue echoes one.
            key = state.modifies.front().copied();
        }
        let State {
            orders,
            by_venue,
            modifies,
            ..
        } = &mut *state;
        let Some((key, tracked)) = key.and_then(|key| Some((key, orders.get_mut(&key)?))) else {
            drop(state);
            return self.report_untracked(row, ts_init);
        };
        if tracked.closed {
            return;
        }
        if let Some(id) = row_id
            && (kind == QfexStatusKind::Modified || tracked.venue_order_id.is_none())
        {
            tracked.venue_order_id = Some(id);
            by_venue.insert(id, key);
        }
        // A row about an id the order had before a modify says nothing about the order now.
        if let Some(id) = row_id
            && Some(id) != tracked.venue_order_id
            && by_venue.get(&id) == Some(&key)
        {
            log::debug!("QFEX {reason} row for the replaced order id {id} of {key} ignored");
            return;
        }

        let modify_rejected = |modifies: &mut VecDeque<ClientOrderId>, tracked: &Tracked| {
            modifies.retain(|id| *id != key);
            self.emitter.emit_order_modify_rejected(
                &tracked.order,
                tracked.venue_order_id,
                reason,
                ts_event,
            );
        };
        match kind {
            QfexStatusKind::Accepted => self.accept(tracked, ts_event),
            QfexStatusKind::Modified => {
                modifies.retain(|id| *id != key);
                self.accept(tracked, ts_event);
                self.updated(tracked, row, ts_event);
            }
            QfexStatusKind::Filled => {
                self.accept(tracked, ts_event);
                // The fill itself comes on the `fills` channel, also after this row.
                tracked.closed = row.quantity_remaining.is_some_and(|left| left.is_zero());
            }
            QfexStatusKind::FilledRestCanceled | QfexStatusKind::Canceled => {
                self.accept(tracked, ts_event);
                tracked.closed = true;
                self.emitter
                    .emit_order_canceled(&tracked.order, tracked.venue_order_id, ts_event);
            }
            QfexStatusKind::ModifyRejected => modify_rejected(modifies, tracked),
            QfexStatusKind::CancelRejected => self.emitter.emit_order_cancel_rejected(
                &tracked.order,
                tracked.venue_order_id,
                reason,
                ts_event,
            ),
            QfexStatusKind::Rejected if !tracked.accepted => {
                tracked.closed = true;
                // The venue has no status for a post-only order that would cross on entry.
                self.emitter
                    .emit_order_rejected(&tracked.order, reason, ts_event, false);
            }
            // The order rests, so the refusal is about the modify that is in flight.
            QfexStatusKind::Rejected if modifies.contains(&key) => {
                modify_rejected(modifies, tracked);
            }
            QfexStatusKind::Rejected => {
                log::warn!("QFEX refused a request for the open order {key}: {reason}");
            }
        }
    }

    fn report_untracked(&self, row: &QfexWsOrder, ts_init: UnixNanos) {
        match self.order_report(row, ts_init) {
            Ok(Some(report)) => self.emitter.send_order_status_report(report),
            Ok(None) => log::warn!(
                "QFEX answered {} for the unknown order '{}'",
                row.status.as_ref(),
                row.order_id
            ),
            Err(e) => log::warn!("QFEX order row unreadable: {e}"),
        }
    }

    /// The status report of an order row; `None` for a row that only refuses a request.
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol is unknown or a value does not fit the instrument.
    pub fn order_report(
        &self,
        row: &QfexWsOrder,
        ts_init: UnixNanos,
    ) -> anyhow::Result<Option<OrderStatusReport>> {
        let kind = row.status.kind();
        if matches!(
            kind,
            QfexStatusKind::Rejected
                | QfexStatusKind::ModifyRejected
                | QfexStatusKind::CancelRejected
        ) {
            return Ok(None);
        }
        let instrument = self
            .instruments
            .get(&row.symbol)
            .with_context(|| format!("no instrument for symbol {}", row.symbol))?;
        let venue_order_id = checked(VenueOrderId::new_checked(&row.order_id))?;
        let quantity = row.quantity.context("order row without quantity")?;
        let left = row.quantity_remaining.unwrap_or(quantity);
        let order_status = match kind {
            QfexStatusKind::FilledRestCanceled | QfexStatusKind::Canceled => OrderStatus::Canceled,
            _ if left.is_zero() => OrderStatus::Filled,
            _ if left < quantity => OrderStatus::PartiallyFilled,
            _ => OrderStatus::Accepted,
        };
        let venue_type = row.order_type.unwrap_or(QfexOrderType::Limit);
        let (order_type, post_only) = venue_type.as_order_type();
        let ts_last = match row.update_time {
            Some(time) => unix_time_to_nanos(time)?,
            None => ts_init,
        };
        let client_order_id = ClientOrderId::new_checked(&row.client_order_id)
            .ok()
            .or_else(|| self.client_order_id(&venue_order_id));

        let mut report = OrderStatusReport::new(
            self.account_id,
            instrument.id(),
            client_order_id,
            venue_order_id,
            row.side.map(|side| side.as_order_side()),
            order_type,
            row.time_in_force
                .unwrap_or(QfexTimeInForce::Gtc)
                .as_time_in_force(),
            order_status,
            quantity_from_decimal(quantity, instrument.size_precision())?,
            quantity_from_decimal(
                quantity
                    .checked_sub(left)
                    .context("filled quantity is out of range")?,
                instrument.size_precision(),
            )?,
            ts_last,
            ts_last,
            ts_init,
            None,
        )
        .with_post_only(post_only)
        .with_reduce_only(row.reduce_only.unwrap_or(false));
        if let Some(price) = row.price.filter(|price| *price > Decimal::ZERO) {
            let price = price_from_decimal(price, instrument.price_precision())?;
            // The `price` of a stop order is its trigger price.
            report = if venue_type.is_stop() {
                report
                    .with_trigger_price(price)
                    .with_trigger_type(TriggerType::MarkPrice)
            } else {
                report.with_price(price)
            };
        }
        Ok(Some(report))
    }

    /// Handles one `fill_response`.
    pub fn handle_fill(&self, fill: &QfexWsFill, ts_init: UnixNanos) {
        if let Err(e) = self.try_fill(fill, ts_init) {
            // Not marked as seen, so the fill reports of a reconciliation still bring it.
            log::error!("QFEX fill '{}' unreadable: {e}", fill.trade_id);
        }
    }

    fn try_fill(&self, fill: &QfexWsFill, ts_init: UnixNanos) -> anyhow::Result<()> {
        let instrument = self
            .instruments
            .get(&fill.symbol)
            .with_context(|| format!("no instrument for symbol {}", fill.symbol))?;
        let venue_order_id = checked(VenueOrderId::new_checked(&fill.order_id))?;
        let trade_id = checked(TradeId::new_checked(&fill.trade_id))?;
        let last_qty = quantity_from_decimal(fill.quantity, instrument.size_precision())?;
        let last_px = price_from_decimal(fill.price, instrument.price_precision())?;
        // Signed: a negative fee is a rebate. Assumed to be in the margin asset.
        let commission = checked(Money::from_decimal(
            fill.fee,
            instrument.settlement_currency(),
        ))?;
        let liquidity_side = match fill.aggressor_side {
            Some(side) if side == fill.side => LiquiditySide::Taker,
            Some(_) => LiquiditySide::Maker,
            None => liquidity_side_of(fill.order_type),
        };
        let ts_event = unix_time_to_nanos(fill.timestamp)?;

        let mut state = self.state.lock();
        if !state.mark_seen(trade_id) {
            return Ok(());
        }
        let key = state.key_of(&fill.client_order_id, &fill.order_id);
        let State {
            orders, by_venue, ..
        } = &mut *state;
        match key.and_then(|key| Some((key, orders.get_mut(&key)?))) {
            Some((key, tracked)) => {
                if tracked.venue_order_id.is_none() {
                    tracked.venue_order_id = Some(venue_order_id);
                    by_venue.insert(venue_order_id, key);
                }
                self.accept(tracked, ts_event);
                self.emitter.emit_order_filled(
                    &tracked.order,
                    venue_order_id,
                    None,
                    trade_id,
                    last_qty,
                    last_px,
                    instrument.quote_currency(),
                    Some(commission),
                    liquidity_side,
                    ts_event,
                );
            }
            None => self.emitter.send_fill_report(FillReport::new(
                self.account_id,
                instrument.id(),
                venue_order_id,
                trade_id,
                fill.side.as_order_side(),
                last_qty,
                last_px,
                commission,
                liquidity_side,
                ClientOrderId::new_checked(&fill.client_order_id).ok(),
                None,
                ts_event,
                ts_init,
                None,
            )),
        }
        Ok(())
    }

    /// Handles an error frame that sends the refused request back. Returns `false` if the
    /// request is not about a tracked order, so the caller can log it.
    pub fn handle_error(&self, error: &QfexWsError, ts_init: UnixNanos) -> bool {
        let params = &error.incoming_message["params"];
        let reason = format!(
            "{}: {}",
            error.error_code.as_ref().map_or("error", AsRef::as_ref),
            error.message
        );

        let mut state = self.state.lock();
        if error.request_type() == Some("cancel_all_orders") {
            // Every order the refused mass cancel was for stays open. Stop orders have their
            // own cancel (see `cancel_all_orders` of the client).
            let symbol = params["symbol"].as_str();
            let mut any = false;
            for tracked in state.orders.values().filter(|tracked| {
                let order = &tracked.order;
                !tracked.closed
                    && !is_stop(order.order_type())
                    && symbol.is_none_or(|symbol| {
                        venue_symbol_of(&order.instrument_id()).ok() == Some(symbol)
                    })
            }) {
                any = true;
                self.emitter.emit_order_cancel_rejected(
                    &tracked.order,
                    tracked.venue_order_id,
                    &reason,
                    ts_init,
                );
            }
            return any;
        }
        let key = ["client_order_id", "order_id", "stop_order_id"]
            .into_iter()
            .filter_map(|name| params[name].as_str())
            .find_map(|id| state.key_of(id, id));
        let State {
            orders, modifies, ..
        } = &mut *state;
        let Some((key, tracked)) = key
            .and_then(|key| Some((key, orders.get_mut(&key)?)))
            .filter(|(_, tracked)| !tracked.closed)
        else {
            return false;
        };
        let (order, venue_order_id) = (&tracked.order, tracked.venue_order_id);
        match error.request_type() {
            Some("add_order") if !tracked.accepted => {
                tracked.closed = true;
                self.emitter
                    .emit_order_rejected(order, &reason, ts_init, false);
            }
            Some("modify_order" | "modify_stop_order") => {
                modifies.retain(|id| *id != key);
                self.emitter
                    .emit_order_modify_rejected(order, venue_order_id, &reason, ts_init);
            }
            Some("cancel_order" | "cancel_stop_order") => {
                self.emitter
                    .emit_order_cancel_rejected(order, venue_order_id, &reason, ts_init);
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use nautilus_common::messages::{ExecutionEvent, ExecutionReport};
    use nautilus_core::time::get_atomic_clock_realtime;
    use nautilus_model::{
        enums::{AccountType, OrderType},
        events::OrderEventAny,
        identifiers::TraderId,
        types::{Price, Quantity},
    };
    use rstest::rstest;
    use serde_json::{Value, json};
    use tokio::sync::mpsc::UnboundedReceiver;

    use super::*;
    use crate::{
        execution::order::tests::builder,
        http::parse::tests::instrument,
        websocket::messages::{QfexWsMessage, tests::first},
    };

    const OLD_ID: &str = "5b309929-206f-40ec-804d-cbe46e81afc1";
    const NEW_ID: &str = "88888888-4444-4444-4444-1234567890ab";

    fn dispatch() -> (OrderDispatch, UnboundedReceiver<ExecutionEvent>) {
        let account_id = AccountId::from("QFEX-001");
        let mut emitter = ExecutionEventEmitter::new(
            get_atomic_clock_realtime(),
            TraderId::from("TESTER-001"),
            account_id,
            AccountType::Margin,
            None,
        );
        let (sender, events) = tokio::sync::mpsc::unbounded_channel();
        emitter.set_sender(sender);
        let instruments = ["AAPL-USD", "US100-USD"]
            .map(|symbol| (Ustr::from(symbol), instrument(symbol)))
            .into();
        (OrderDispatch::new(account_id, emitter, instruments), events)
    }

    fn limit() -> OrderAny {
        builder(OrderType::Limit)
            .price(Price::from("200.25"))
            .build()
    }

    fn order_event(events: &mut UnboundedReceiver<ExecutionEvent>) -> OrderEventAny {
        match events.try_recv() {
            Ok(ExecutionEvent::Order(event)) => event,
            other => panic!("expected an order event, got {other:?}"),
        }
    }

    fn row(dispatch: &OrderDispatch, row: Value) {
        let QfexWsMessage::Order(row) =
            QfexWsMessage::parse(&json!({"order_response": row}).to_string()).unwrap()
        else {
            panic!("not an order row");
        };
        dispatch.handle_order(&row, UnixNanos::from(1));
    }

    fn fill(dispatch: &OrderDispatch, trade_id: &str) {
        let frame = json!({"fill_response": {
            "trade_id": trade_id, "order_id": NEW_ID, "symbol": "AAPL-USD", "price": 201.0,
            "quantity": 0.5, "side": "BUY", "aggressor_side": "SELL", "fee": -0.01,
            "order_type": "LIMIT", "timestamp": 1_769_439_642.5,
        }});
        let QfexWsMessage::Fill(fill) = QfexWsMessage::parse(&frame.to_string()).unwrap() else {
            panic!("not a fill");
        };
        dispatch.handle_fill(&fill, UnixNanos::from(1));
    }

    #[rstest]
    fn test_tracked_order_gets_events_through_a_modify_that_changes_its_id() {
        let (dispatch, mut events) = dispatch();
        let order = limit();
        dispatch.track(&order);
        dispatch.track(&order);

        let ack = json!({"order_id": OLD_ID, "client_order_id": "O-1", "symbol": "AAPL-USD",
            "status": "ACK", "quantity": 1.5, "price": 200.25});
        row(&dispatch, ack.clone());
        row(&dispatch, ack);
        let OrderEventAny::Accepted(accepted) = order_event(&mut events) else {
            panic!("not accepted");
        };
        assert_eq!(accepted.venue_order_id, VenueOrderId::from(OLD_ID));

        // As in the docs: the answer to a modify has a new order id and no client order id.
        dispatch.begin_modify(order.client_order_id());
        row(
            &dispatch,
            json!({"order_id": NEW_ID, "client_order_id": "", "symbol": "AAPL-USD",
                "status": "MODIFIED", "quantity": 2.0, "price": 201.0}),
        );
        let OrderEventAny::Updated(updated) = order_event(&mut events) else {
            panic!("not updated");
        };
        assert_eq!(updated.venue_order_id, Some(VenueOrderId::from(NEW_ID)));
        assert_eq!(updated.quantity, Quantity::from("2.00"));
        assert_eq!(updated.price, Some(Price::from("201.00")));
        assert_eq!(
            dispatch.venue_order_id(&order.client_order_id()),
            Some(VenueOrderId::from(NEW_ID))
        );

        // A cancel of the replaced id is not a cancel of the order.
        row(
            &dispatch,
            json!({"order_id": OLD_ID, "symbol": "AAPL-USD", "status": "CANCELLED"}),
        );

        fill(&dispatch, "T-1");
        fill(&dispatch, "T-1");
        let OrderEventAny::Filled(filled) = order_event(&mut events) else {
            panic!("not filled");
        };
        assert_eq!(filled.client_order_id, order.client_order_id());
        assert_eq!(filled.venue_order_id, VenueOrderId::from(NEW_ID));
        assert_eq!(filled.last_px, Price::from("201.00"));
        assert_eq!(filled.liquidity_side, LiquiditySide::Maker);
        assert_eq!(filled.commission.unwrap().as_decimal(), Decimal::new(-1, 2));
        assert_eq!(
            filled.ts_event,
            UnixNanos::from(1_769_439_642_500_000_000_u64)
        );

        let cancelled = json!({"order_id": NEW_ID, "symbol": "AAPL-USD", "status": "CANCELLED"});
        row(&dispatch, cancelled.clone());
        row(&dispatch, cancelled);
        assert!(matches!(
            order_event(&mut events),
            OrderEventAny::Canceled(_)
        ));
        assert!(events.try_recv().is_err());
    }

    #[rstest]
    fn test_refusals_reach_the_order_they_are_about() {
        let (dispatch, mut events) = dispatch();
        let order = limit();
        dispatch.track(&order);
        let error = |request: &str, params: Value| {
            let frame = json!({"err": {"error_code": "InvalidOrder", "message": "bad price",
                "incoming_message": {"type": request, "params": params}}});
            let QfexWsMessage::Error(error) = QfexWsMessage::parse(&frame.to_string()).unwrap()
            else {
                panic!("not an error");
            };
            error
        };

        let other = error("add_order", json!({"client_order_id": "someone-else"}));
        assert!(!dispatch.handle_error(&other, UnixNanos::from(1)));

        let refused = error("add_order", json!({"client_order_id": "O-1"}));
        assert!(dispatch.handle_error(&refused, UnixNanos::from(1)));
        let OrderEventAny::Rejected(rejected) = order_event(&mut events) else {
            panic!("not rejected");
        };
        assert_eq!(rejected.reason.as_str(), "InvalidOrder: bad price");
        // The order is closed: nothing more is said about it.
        assert!(!dispatch.handle_error(&refused, UnixNanos::from(1)));

        let resting = builder(OrderType::Limit)
            .client_order_id(ClientOrderId::from("O-2"))
            .price(Price::from("200.25"))
            .build();
        dispatch.track(&resting);
        row(
            &dispatch,
            json!({"order_id": OLD_ID, "client_order_id": "O-2", "symbol": "AAPL-USD",
                "status": "ACK", "quantity": 1.5, "price": 200.25}),
        );
        assert!(matches!(
            order_event(&mut events),
            OrderEventAny::Accepted(_)
        ));
        let modify = error("modify_order", json!({"order_id": OLD_ID}));
        assert!(dispatch.handle_error(&modify, UnixNanos::from(1)));
        assert!(matches!(
            order_event(&mut events),
            OrderEventAny::ModifyRejected(_)
        ));
        // A cancel by client order id carries it in `order_id`.
        let cancel = error("cancel_order", json!({"order_id": "O-2"}));
        assert!(dispatch.handle_error(&cancel, UnixNanos::from(1)));
        assert!(matches!(
            order_event(&mut events),
            OrderEventAny::CancelRejected(_)
        ));
        assert!(events.try_recv().is_err());

        // A refused mass cancel: the open order of that symbol stays open, the closed one and
        // other symbols are left alone.
        let other_symbol = error("cancel_all_orders", json!({"symbol": "US100-USD"}));
        assert!(!dispatch.handle_error(&other_symbol, UnixNanos::from(1)));
        let mass = error("cancel_all_orders", json!({"symbol": "AAPL-USD"}));
        assert!(dispatch.handle_error(&mass, UnixNanos::from(1)));
        let OrderEventAny::CancelRejected(rejected) = order_event(&mut events) else {
            panic!("not cancel rejected");
        };
        assert_eq!(rejected.client_order_id, ClientOrderId::from("O-2"));
        assert!(events.try_recv().is_err());
    }

    #[rstest]
    fn test_rows_of_other_sessions_become_reports() {
        let (dispatch, mut events) = dispatch();

        let ack = first("trade", |message| match message {
            QfexWsMessage::Order(row) => Some(row),
            _ => None,
        });
        dispatch.handle_order(&ack, UnixNanos::from(1));
        let Ok(ExecutionEvent::Report(ExecutionReport::Order(report))) = events.try_recv() else {
            panic!("no order report");
        };
        assert_eq!(report.venue_order_id, VenueOrderId::from(OLD_ID));
        assert_eq!(report.order_status, OrderStatus::Accepted);
        assert_eq!(report.client_order_id, None);
        assert_eq!(report.price, Some(Price::from("200.00")));

        let fill = first("trade", |message| match message {
            QfexWsMessage::Fill(fill) => Some(fill),
            _ => None,
        });
        dispatch.handle_fill(&fill, UnixNanos::from(1));
        dispatch.handle_fill(&fill, UnixNanos::from(1));
        let Ok(ExecutionEvent::Report(ExecutionReport::Fill(report))) = events.try_recv() else {
            panic!("no fill report");
        };
        assert_eq!(report.liquidity_side, LiquiditySide::Taker);
        assert_eq!(report.last_px.as_decimal(), Decimal::new(25242, 0));
        assert!(events.try_recv().is_err());
        assert!(!dispatch.is_new_trade(report.trade_id));
    }
}
