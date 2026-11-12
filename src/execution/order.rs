//! Builds the trade-socket requests of an order. No I/O.

use nautilus_model::{
    enums::{OrderType, TimeInForce},
    identifiers::VenueOrderId,
    orders::{Order, OrderAny},
    types::{Price, Quantity},
};
use ustr::Ustr;

use crate::{
    common::enums::{QfexOrderType, QfexSide, QfexTimeInForce},
    websocket::messages::{
        QfexAddOrder, QfexCancelIdType, QfexModifyOrder, QfexModifyStopOrder, QfexTradeRequest,
    },
};

/// The venue order type and time in force of an order (instruction table of the docs:
/// market and stop orders are IOC only, ALO is GTC only).
fn venue_type(order: &OrderAny) -> anyhow::Result<(QfexOrderType, QfexTimeInForce)> {
    let tif = order.time_in_force();
    Ok(match order.order_type() {
        OrderType::Market => {
            // A Nautilus market order is GTC unless told otherwise, so only FOK is refused.
            anyhow::ensure!(tif != TimeInForce::Fok, "QFEX market orders are IOC only");
            (QfexOrderType::Market, QfexTimeInForce::Ioc)
        }
        OrderType::Limit if order.is_post_only() => {
            anyhow::ensure!(
                tif == TimeInForce::Gtc,
                "QFEX post-only (ALO) orders are GTC only"
            );
            (QfexOrderType::Alo, QfexTimeInForce::Gtc)
        }
        OrderType::Limit => (QfexOrderType::Limit, QfexTimeInForce::try_from(tif)?),
        OrderType::StopMarket => (QfexOrderType::StopLoss, QfexTimeInForce::Ioc),
        OrderType::MarketIfTouched => (QfexOrderType::TakeProfit, QfexTimeInForce::Ioc),
        other => anyhow::bail!("order type {other} is not supported by QFEX"),
    })
}

/// Whether the venue holds the order as a stop order (own id space, own cancel and modify).
#[must_use]
pub fn is_stop(order_type: OrderType) -> bool {
    matches!(
        order_type,
        OrderType::StopMarket | OrderType::MarketIfTouched
    )
}

/// `add_order` for a new order. For a stop order `price` is the trigger price.
///
/// # Errors
///
/// Returns an error if the venue has no such order type, time in force or quantity kind.
pub fn add_order(order: &OrderAny, symbol: Ustr) -> anyhow::Result<QfexAddOrder> {
    anyhow::ensure!(
        !order.is_quote_quantity(),
        "QFEX takes the quantity in contracts, not in quote currency"
    );
    let (order_type, order_time_in_force) = venue_type(order)?;
    let price = match order_type {
        QfexOrderType::Market => None,
        _ if order_type.is_stop() => Some(
            order
                .trigger_price()
                .ok_or_else(|| anyhow::anyhow!("stop order without trigger price"))?,
        ),
        _ => Some(
            order
                .price()
                .ok_or_else(|| anyhow::anyhow!("limit order without price"))?,
        ),
    };
    Ok(QfexAddOrder {
        symbol,
        side: QfexSide::try_from(order.order_side())?,
        order_type,
        order_time_in_force,
        quantity: order.quantity().as_decimal(),
        price: price.map(|price| price.as_decimal()),
        client_order_id: order.client_order_id().to_string(),
        reduce_only: order.is_reduce_only(),
    })
}

/// `modify_order`, or `modify_stop_order` for a stop order. `price` is the new limit price,
/// or the new trigger price of a stop order.
///
/// # Errors
///
/// Returns an error if the order cannot be modified at the venue.
pub fn modify_order(
    order: &OrderAny,
    venue_order_id: &VenueOrderId,
    symbol: Ustr,
    quantity: Quantity,
    price: Price,
) -> anyhow::Result<QfexTradeRequest> {
    let (order_type, _) = venue_type(order)?;
    anyhow::ensure!(
        order_type != QfexOrderType::Market,
        "market orders cannot be modified"
    );
    let side = QfexSide::try_from(order.order_side())?;
    let (quantity, price) = (quantity.as_decimal(), price.as_decimal());
    Ok(if order_type.is_stop() {
        QfexTradeRequest::ModifyStopOrder(QfexModifyStopOrder {
            stop_order_id: venue_order_id.to_string(),
            symbol,
            side,
            order_type,
            quantity,
            price,
        })
    } else {
        QfexTradeRequest::ModifyOrder(QfexModifyOrder {
            order_id: venue_order_id.to_string(),
            symbol,
            side,
            order_type,
            reduce_only: order.is_reduce_only(),
            quantity,
            price: Some(price),
        })
    })
}

/// `cancel_order` by venue id, or by client id while the venue id is not known;
/// `cancel_stop_order` for a stop order, which the venue cancels by its own id only.
///
/// # Errors
///
/// Returns an error for a stop order without venue order id.
pub fn cancel_order(
    stop: bool,
    symbol: Ustr,
    client_order_id: &str,
    venue_order_id: Option<&VenueOrderId>,
) -> anyhow::Result<QfexTradeRequest> {
    Ok(match (stop, venue_order_id) {
        (true, Some(id)) => QfexTradeRequest::CancelStopOrder {
            stop_order_id: id.to_string(),
        },
        (true, None) => anyhow::bail!("stop order has no venue order id yet"),
        (false, Some(id)) => QfexTradeRequest::CancelOrder {
            order_id: id.to_string(),
            symbol,
            cancel_order_id_type: QfexCancelIdType::OrderId,
        },
        (false, None) => QfexTradeRequest::CancelOrder {
            order_id: client_order_id.to_string(),
            symbol,
            cancel_order_id_type: QfexCancelIdType::ClientOrderId,
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use nautilus_model::{
        enums::{OrderSide, TriggerType},
        identifiers::{ClientOrderId, InstrumentId},
        orders::OrderTestBuilder,
    };
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    pub(crate) fn builder(order_type: OrderType) -> OrderTestBuilder {
        let mut builder = OrderTestBuilder::new(order_type);
        builder
            .instrument_id(InstrumentId::from("AAPL-USD-PERP.QFEX"))
            .client_order_id(ClientOrderId::from("O-1"))
            .side(OrderSide::Buy)
            .quantity(Quantity::from("1.50"));
        builder
    }

    fn symbol() -> Ustr {
        Ustr::from("AAPL-USD")
    }

    fn frame(request: &QfexTradeRequest) -> serde_json::Value {
        serde_json::from_str(&request.to_frame()).unwrap()
    }

    #[rstest]
    fn test_add_order_frames() {
        let market = builder(OrderType::Market).build();
        assert_eq!(
            frame(&QfexTradeRequest::AddOrder(
                add_order(&market, symbol()).unwrap()
            )),
            json!({"type": "add_order", "params": {
                "symbol": "AAPL-USD", "side": "BUY", "order_type": "MARKET",
                "order_time_in_force": "IOC", "quantity": 1.5, "client_order_id": "O-1",
            }})
        );

        let alo = builder(OrderType::Limit)
            .price(Price::from("200.25"))
            .post_only(true)
            .reduce_only(true)
            .build();
        assert_eq!(
            frame(&QfexTradeRequest::AddOrder(
                add_order(&alo, symbol()).unwrap()
            )),
            json!({"type": "add_order", "params": {
                "symbol": "AAPL-USD", "side": "BUY", "order_type": "ALO",
                "order_time_in_force": "GTC", "quantity": 1.5, "price": 200.25,
                "client_order_id": "O-1", "reduce_only": true,
            }})
        );

        let limit = builder(OrderType::Limit)
            .price(Price::from("200.25"))
            .time_in_force(TimeInForce::Fok)
            .build();
        let request = add_order(&limit, symbol()).unwrap();
        assert_eq!(
            (request.order_type, request.order_time_in_force),
            (QfexOrderType::Limit, QfexTimeInForce::Fok)
        );
    }

    #[rstest]
    #[case(OrderType::StopMarket, QfexOrderType::StopLoss)]
    #[case(OrderType::MarketIfTouched, QfexOrderType::TakeProfit)]
    fn test_stop_orders_carry_the_trigger_price(
        #[case] order_type: OrderType,
        #[case] expected: QfexOrderType,
    ) {
        let order = builder(order_type)
            .side(OrderSide::Sell)
            .trigger_price(Price::from("150.00"))
            .trigger_type(TriggerType::MarkPrice)
            .build();
        let request = add_order(&order, symbol()).unwrap();
        assert_eq!(request.order_type, expected);
        assert_eq!(request.order_time_in_force, QfexTimeInForce::Ioc);
        assert_eq!(request.price, Some(rust_decimal_macros::dec!(150.00)));
        assert_eq!(request.side, QfexSide::Sell);
    }

    #[rstest]
    fn test_what_the_venue_cannot_do_is_refused() {
        let fok_market = builder(OrderType::Market)
            .time_in_force(TimeInForce::Fok)
            .build();
        assert!(add_order(&fok_market, symbol()).is_err());

        let ioc_post_only = builder(OrderType::Limit)
            .price(Price::from("200.25"))
            .post_only(true)
            .time_in_force(TimeInForce::Ioc)
            .build();
        assert!(add_order(&ioc_post_only, symbol()).is_err());

        let stop_limit = builder(OrderType::StopLimit)
            .price(Price::from("200.25"))
            .trigger_price(Price::from("200.00"))
            .build();
        let error = add_order(&stop_limit, symbol()).unwrap_err();
        assert!(error.to_string().contains("not supported"), "{error}");

        let quote_quantity = builder(OrderType::Market).quote_quantity(true).build();
        assert!(add_order(&quote_quantity, symbol()).is_err());
    }

    #[rstest]
    fn test_modify_and_cancel_frames() {
        let venue_order_id = VenueOrderId::from("5b309929-206f-40ec-804d-cbe46e81afc1");
        let (quantity, price) = (Quantity::from("2.00"), Price::from("201.00"));

        let limit = builder(OrderType::Limit)
            .price(Price::from("200.25"))
            .build();
        let request = modify_order(&limit, &venue_order_id, symbol(), quantity, price).unwrap();
        assert_eq!(
            frame(&request),
            json!({"type": "modify_order", "params": {
                "order_id": "5b309929-206f-40ec-804d-cbe46e81afc1", "symbol": "AAPL-USD",
                "side": "BUY", "order_type": "LIMIT", "reduce_only": false,
                "quantity": 2.0, "price": 201.0,
            }})
        );

        let stop = builder(OrderType::StopMarket)
            .trigger_price(Price::from("150.00"))
            .build();
        let request = modify_order(&stop, &venue_order_id, symbol(), quantity, price).unwrap();
        assert_eq!(
            frame(&request),
            json!({"type": "modify_stop_order", "params": {
                "stop_order_id": "5b309929-206f-40ec-804d-cbe46e81afc1", "symbol": "AAPL-USD",
                "side": "BUY", "order_type": "STOP_LOSS", "quantity": 2.0, "price": 201.0,
            }})
        );

        let market = builder(OrderType::Market).build();
        assert!(modify_order(&market, &venue_order_id, symbol(), quantity, price).is_err());

        assert_eq!(
            frame(&cancel_order(false, symbol(), "O-1", Some(&venue_order_id)).unwrap()),
            json!({"type": "cancel_order", "params": {
                "order_id": "5b309929-206f-40ec-804d-cbe46e81afc1", "symbol": "AAPL-USD",
                "cancel_order_id_type": "order_id",
            }})
        );
        assert_eq!(
            frame(&cancel_order(false, symbol(), "O-1", None).unwrap()),
            json!({"type": "cancel_order", "params": {
                "order_id": "O-1", "symbol": "AAPL-USD",
                "cancel_order_id_type": "client_order_id",
            }})
        );
        assert_eq!(
            frame(&cancel_order(true, symbol(), "O-1", Some(&venue_order_id)).unwrap()),
            json!({"type": "cancel_stop_order", "params": {
                "stop_order_id": "5b309929-206f-40ec-804d-cbe46e81afc1",
            }})
        );
        assert!(cancel_order(true, symbol(), "O-1", None).is_err());
    }
}
