# Evidence · UAT run 1 · 28 Nov 2026

Run by the agent at the person's request in this session ("run uat ให้หน่อยเพราะให้ apikey ไปแล้ว"), with the UAT key the person put in the main checkout's `.env` (ignored by git, never committed). The key was loaded into the command's environment only, and the output was passed through `sed` to mask it. Nothing went to mainnet. User and account ids are left out below.

| Step | When (UTC) | Command | Exit | Result |
| --- | --- | --- | --- | --- |
| 1 and 2, first run | 12:23 | `cargo run --example exec_tester --features examples -- 2` (SIGINT after 60 s) | 0 | `/user/positions` 200; login answer `{"authenticated":true}` not read, connect failed after 10 s |
| 1 and 2, after `26750aa` | 12:26 | the same | 0 | login, subscribe, `get_user_orders`, reconciliation (0 orders, 0 fills, 0 positions); no order (dry run) |
| 3 | 12:29 | `cargo run --example exec_tester --features examples -- 3` (SIGINT after 75 s) | 0 | no quote in 67 s, no order sent; `AAPL-USD` in its `weekend` session |

Frames of the second step 2 run, from `recordings/uat-step-2.jsonl` (kept on the machine, ignored by git):

```text
REST GET /refdata 200
REST GET /user/positions 200
sent {"type":"auth"}
received {"authenticated":true}
sent {"type":"subscribe","params":{"channels":["order_responses","fills","positions","balances"]}}
sent {"type":"get_user_orders","params":{"limit":1000,"offset":0}}
received {"subscribed":"order_responses"}
received balance_response with keys ['available_balance', 'builder_rewards', 'deposit', 'fees', 'id', 'net_funding', 'order_margin', 'position_margin', 'realised_pnl', 'referral_rewards', 'unrealised_pnl', 'user_id']
received {"all_orders_response":{"orders":[],"twaps":[]}}
REST GET /user/trade 200
REST GET /user/historic-orders 200
```

UAT book of `AAPL-USD` at 12:30 UTC (`GET /md/orderbook/AAPL-USD`, public): bids `[["50","1"]]`, asks none. `market_hours.saturday` of `/refdata`: `[{"session":"weekend","open":"00:00:00","close":"23:59:59"}]`.
