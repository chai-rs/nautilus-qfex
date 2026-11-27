FR: FR-4
KIND: request
GOAL: G2

The order path (FR-4 submit, modify, cancel; FR-5 order, fill and account events; FR-6 reports) has run against QFEX UAT, not only against the hand-made mock.

## What is there now

- `docs/STATUS.md` marks authentication and execution UNVERIFIED on the venue. Every execution test runs against `tests/exec_client.rs`, whose frames were written by hand from the docs.
- `docs/STATUS.md`, section "The UAT gate", lists nine steps: HMAC on `GET /user/positions`, trade socket login, post-only submit and cancel, modify, one small market order and its fill, one stop order, drop with an open order, record real frames into fixtures, `qfex-flatten uat`.
- `docs/PLAN.md` "Open questions" lists what these steps should answer (ACK echo of `client_order_id`, `CANCELLED` after `MODIFIED`, stop order listing, refused login frame, error frame shapes).

## What is asked

- Run the UAT gate against `qfex.io` with a UAT key, record what came back, answer the open questions it covers, and replace hand-made fixtures with recorded frames where they differ.
- Update `docs/STATUS.md` and G2 in `ARCHITECTURE.md` with the result.

## Constraints

- **This needs a person.** It sends authenticated requests and orders to QFEX UAT. `AGENTS.md` forbids an agent from sending authenticated requests or orders to QFEX on its own; a person has to provide the UAT key (`QFEX_UAT_API_KEY`, `QFEX_UAT_API_SECRET`) and approve each step that leaves this machine.
- UAT only. Nothing goes to mainnet.
- The key never lands in git, a log line or a fixture.
