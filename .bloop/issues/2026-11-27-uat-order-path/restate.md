CONFIRMED: yes

# The order path has run on QFEX UAT

## What we will do

- First slice, agent work, no key needed: `exec_tester` gains an option that writes every REST answer and every trade-socket frame it sends and receives to a file, one line each, with the API key, the secret and the HMAC signature never written. A test against the local mock proves the file holds the frames and holds no key, secret or signature.
- The person runs the nine steps of "The UAT gate" in `docs/STATUS.md` on `qfex.io` with their UAT key, approving each step, with the recording option on. The agent prepares the exact command for each step and sends nothing to QFEX itself.
- With the recorded file in hand, the agent replaces the hand-made fixtures in `tests/fixtures/` with recorded frames where they differ, fixes the code where the venue behaves differently from the docs, and keeps every test passing.
- `docs/STATUS.md` marks each UAT step done with what came back; `docs/PLAN.md` "Open questions" gets the answers the steps gave (ACK echo of `client_order_id`, `CANCELLED` after `MODIFIED`, stop order listing, refused login frame, error frame shapes); G2 in `ARCHITECTURE.md` moves from 0 of 4.

## What we will not do

- Any request to QFEX mainnet.
- An agent sending any authenticated request or order to UAT: the person does.
- Keys in git, logs, fixtures or the recording.
- TWAP, subaccounts, or anything else out of v1 scope.

## Contracts affected

- `docs/external/qfex/README.md` "What we cannot rely on" shrinks by what UAT confirms
