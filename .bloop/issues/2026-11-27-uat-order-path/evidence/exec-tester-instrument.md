# Evidence · exec-tester-instrument · PASS

| | |
| --- | --- |
| Ran | 2026-11-28 12:58:04 +0000 (2026-11-28T12:58:04Z) |
| Command | `./checks.sh` |
| Commit | `3cc8b33` on `test/2026-11-27-uat-order-path`, with uncommitted changes |
| Took | 13s |
| Exit code | 0 |
| Result | **PASS** |

## Summary

Ran `./checks.sh` at 2026-11-28 12:58:04 +0000 against `3cc8b33`, with uncommitted changes. It passed, exit code 0 after 13s. Steps run: format, lint, unit + integration, test count, shortcut marker.

## Output

Exactly what the gate printed, untouched.

```text

==> format

==> lint
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

==> unit + integration
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.44s
     Running unittests src/lib.rs (target/debug/deps/nautilus_qfex-0407fb27e789ca5d)

running 155 tests
test book::tests::test_flags_and_empty_levels ... ok
test book::tests::test_unsorted_input_is_ordered_and_bad_values_are_errors ... ok
test book::tests::test_each_message_replaces_the_book ... ok
test common::credential::tests::test_credential_env_vars ... ok
test book::tests::test_rest_snapshot_depth_and_quote ... ok
test common::credential::tests::test_from_resolved_values ... ok
test common::enums::tests::test_bar_interval::case_1 ... ok
test common::enums::tests::test_bar_interval::case_2 ... ok
test common::enums::tests::test_bar_interval::case_3 ... ok
test common::enums::tests::test_bar_interval::case_4 ... ok
test common::enums::tests::test_bar_interval::case_5 ... ok
test common::credential::tests::test_debug_redacts_api_secret ... ok
test common::credential::tests::test_from_resolved_values_rejects_partial_values ... ok
test common::enums::tests::test_bar_interval::case_6 ... ok
test common::enums::tests::test_order_status_kind::case_01 ... ok
test common::enums::tests::test_order_status_kind::case_04 ... ok
test common::enums::tests::test_order_status_kind::case_02 ... ok
test common::enums::tests::test_order_status_kind::case_05 ... ok
test common::enums::tests::test_order_status_kind::case_03 ... ok
test common::enums::tests::test_order_status_kind::case_06 ... ok
test common::enums::tests::test_order_status_kind::case_07 ... ok
test common::enums::tests::test_order_status_kind::case_08 ... ok
test common::enums::tests::test_order_status_kind::case_09 ... ok
test common::enums::tests::test_order_status_kind::case_10 ... ok
test common::enums::tests::test_order_status_kind::case_11 ... ok
test common::enums::tests::test_order_status_names_round_trip ... ok
test common::enums::tests::test_side_accepts_both_cases::case_1 ... ok
test common::enums::tests::test_side_accepts_both_cases::case_2 ... ok
test common::parse::tests::test_precision_of::case_1 ... ok
test common::parse::tests::test_precision_of::case_2 ... ok
test common::parse::tests::test_precision_of::case_4 ... ok
test common::parse::tests::test_precision_of::case_3 ... ok
test common::parse::tests::test_precision_of_rejects_zero ... ok
test common::parse::tests::test_symbol_round_trip ... ok
test common::parse::tests::test_unix_time_to_nanos::case_1 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_2 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_3 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_4 ... ok
test common::parse::tests::test_unix_time_to_nanos_rejects_bad_values ... ok
test common::enums::tests::test_wire_names ... ok
test common::parse::tests::test_parse_rfc3339 ... ok
test common::recording::tests::other_frames_are_recorded_as_they_are ... ok
test common::recording::tests::rest_answer_carries_request_and_answer ... ok
test common::recording::tests::an_echoed_auth_loses_its_params_at_the_top_and_inside_err ... ok
test common::recording::tests::sent_auth_keeps_only_its_type ... ok
test common::urls::tests::test_urls_per_environment::case_1 ... ok
test common::urls::tests::test_urls_per_environment::case_2 ... ok
test data::tests::deltas_and_depth_share_the_level2_channel ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_1 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_2 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_3 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_4 ... ok
test config::exec::tests::debug_hides_the_secret ... ok
test config::exec::tests::debug_shows_the_record_path ... ok
test execution::order::tests::test_add_order_frames ... ok
test execution::order::tests::test_modify_and_cancel_frames ... ok
test execution::order::tests::test_stop_orders_carry_the_trigger_price::case_1 ... ok
test execution::order::tests::test_stop_orders_carry_the_trigger_price::case_2 ... ok
test execution::order::tests::test_what_the_venue_cannot_do_is_refused ... ok
test execution::dispatch::tests::test_refusals_reach_the_order_they_are_about ... ok
test execution::tests::test_all_pages_stops_at_a_short_page_and_at_the_cap ... ok
test execution::tests::test_only_later_pages_carry_an_offset ... ok
test execution::dispatch::tests::test_tracked_order_gets_events_through_a_modify_that_changes_its_id ... ok
test execution::dispatch::tests::test_rows_of_other_sessions_become_reports ... ok
test fuzzing::tests::test_book_apply_survives_any_levels ... ok
test fuzzing::tests::test_auth_survives_any_strings ... ok
test fuzzing::tests::test_wire_format_survives_25_decimals ... ok
test fuzzing::tests::test_wire_format_survives_any_decimals ... ok
test fuzzing::tests::test_mds_message_survives_mutated_frames ... ok
test fuzzing::tests::test_trade_message_survives_mutated_frames ... ok
test http::client::tests::test_authenticated_request_without_credentials_is_not_sent ... ok
test http::client::tests::test_authenticated_request_is_signed ... ok
test http::client::tests::test_bars_and_funding_are_ascending_and_limited ... ok
test http::client::tests::test_book_snapshot ... ok
test http::client::tests::test_error_statuses::case_1 ... ok
test http::client::tests::test_error_statuses::case_2 ... ok
test http::client::tests::test_iso_and_history_range ... ok
test http::client::tests::test_other_authenticated_endpoints ... ok
test http::client::tests::test_error_statuses::case_3 ... ok
test http::client::tests::test_path_segment::case_1 ... ok
test http::client::tests::test_path_segment::case_2 ... ok
test http::client::tests::test_path_segment::case_3 ... ok
test http::client::tests::test_path_segment::case_4 ... ok
test http::client::tests::test_path_segment::case_5 ... ok
test http::client::tests::test_path_segment::case_6 ... ok
test http::client::tests::test_error_statuses::case_4 ... ok
test http::error::tests::test_retry_after_only_on_http_and_rate_limit ... ok
test http::error::tests::test_should_retry::case_1 ... ok
test http::error::tests::test_should_retry::case_2 ... ok
test http::error::tests::test_should_retry::case_3 ... ok
test http::error::tests::test_should_retry::case_4 ... ok
test http::error::tests::test_should_retry::case_5 ... ok
test http::error::tests::test_should_retry::case_6 ... ok
test http::error::tests::test_should_retry::case_7 ... ok
test http::models::tests::test_authenticated_models ... ok
test http::models::tests::test_candles_and_funding ... ok
test http::models::tests::test_leverage_request_sends_a_number ... ok
test http::models::tests::test_order_book ... ok
test http::client::tests::test_unreadable_body_is_a_parse_error ... ok
test http::client::tests::test_public_request_sends_the_api_version_and_no_credentials ... ok
test http::models::tests::test_refdata ... ok
test http::parse::tests::test_historic_order_status::case_2 ... ok
test http::parse::tests::test_historic_order_status::case_3 ... ok
test http::parse::tests::test_historic_order_status::case_1 ... ok
test http::parse::tests::test_parse_balance ... ok
test http::parse::tests::test_historic_order_status::case_4 ... ok
test http::parse::tests::test_non_usd_quote_keeps_its_currency ... ok
test http::parse::tests::test_parse_bar_widens_the_range ... ok
test http::parse::tests::test_parse_bar ... ok
test http::parse::tests::test_parse_fill_report ... ok
test http::parse::tests::test_parse_historic_order_status_report ... ok
test http::parse::tests::test_parse_funding_rate ... ok
test http::parse::tests::test_parse_position_status_report ... ok
test signing::tests::test_default_source_gives_distinct_short_nonces_and_a_current_time ... ok
test signing::tests::test_headers ... ok
test http::parse::tests::test_parse_instrument_rejects_unusable_values ... ok
test http::parse::tests::test_parse_instruments ... ok
test signing::tests::test_nonce_length::case_2 ... ok
test signing::tests::test_nonce_length::case_1 ... ok
test signing::tests::test_nonce_length::case_3 ... ok
test signing::tests::test_nonce_length::case_4 ... ok
test signing::tests::test_signature_matches_openssl ... ok
test signing::tests::test_ws_message_has_integer_timestamp ... ok
test signing::tests::test_hmac_primitive_matches_rfc_4231 ... ok
test websocket::client::tests::mds_reconnect_request_follows_the_connection ... ok
test websocket::client::tests::mds_reconnect_resends_every_subscription_grouped_by_channel ... ok
test websocket::client::tests::mds_state_sink_sees_connect_loss_and_reconnect ... ok
test websocket::client::tests::mds_subscribe_batches_every_symbol_of_a_channel_into_one_frame ... ok
test websocket::client::tests::mds_subscribe_delivers_ack_then_data_and_answers_ping ... ok
test websocket::client::tests::send_without_a_connection_is_an_error ... ok
test websocket::client::tests::mds_subscribe_gate_holds_frames_past_the_inflight_cap ... ok
test fuzzing::tests::test_rest_response_survives_mutated_bodies ... ok
test websocket::client::tests::mds_subscribe_refused_by_the_venue_is_sent_again ... ok
test websocket::client::tests::trade_socket_authenticates_then_starts_the_session_again_after_reconnect ... ok
test websocket::client::tests::trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in ... ok
test websocket::handler::tests::test_unknown_symbol_is_an_error_and_other_frames_give_nothing ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_1 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_2 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_3 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_4 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_5 ... ok
test websocket::messages::tests::test_every_captured_mds_frame_parses ... ok
test websocket::messages::tests::test_every_trade_socket_frame_of_the_docs_parses ... ok
test websocket::messages::tests::test_failed_auth_is_an_error::case_1 ... ok
test websocket::messages::tests::test_failed_auth_is_an_error::case_2 ... ok
test websocket::messages::tests::test_login_answer_recorded_on_uat_is_authenticated ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_1 ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_2 ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_3 ... ok
test websocket::messages::tests::test_mds_fields ... ok
test websocket::messages::tests::test_requests_match_the_docs ... ok
test websocket::messages::tests::test_trade_socket_fields ... ok
test websocket::handler::tests::test_captured_frames_give_every_data_type ... ok
test websocket::client::tests::trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth ... ok
test websocket::client::tests::trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket ... ok

test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.20s

     Running unittests bin/flatten.rs (target/debug/deps/qfex_flatten-c42980948f9c5b96)

running 8 tests
test tests::test_outcome_tells_a_refusal_from_progress::case_4 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_2 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_1 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_3 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_5 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_6 ... ok
test tests::test_plan_sends_only_what_is_needed ... ok
test tests::test_plan_cancels_first_then_closes_every_position ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running unittests bin/trades_probe.rs (target/debug/deps/qfex_trades_probe-cbe2b5dac9cd1532)

running 2 tests
test tests::test_arguments_give_the_venue_symbol_and_the_limit ... ok
test tests::test_trade_line_of_a_recorded_frame ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/data_client.rs (target/debug/deps/data_client-561e72214fc20b21)

running 5 tests
test instrument_status_comes_from_refdata ... ok
test book_depth_is_ten_levels_of_the_pulsed_book ... ok
test connect_subscribe_reconnect_request_and_disconnect ... ok
test reconnect_command_reconnects_the_market_data_socket ... ok
test socket_state_follows_the_market_data_socket ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.09s

     Running tests/exec_client.rs (target/debug/deps/exec_client-a7b171161ddf731d)

running 14 tests
test leverage_is_set_at_connect_and_on_request ... ok
test cancel_all_cancels_the_orders_of_the_strategy_one_by_one ... ok
test modify_follows_the_new_order_id ... ok
test cancel_all_without_a_side_uses_the_mass_cancel_when_it_hits_nothing_else ... ok
test connect_signs_rest_logs_in_and_subscribes ... ok
test order_is_accepted_filled_and_cancelled ... ok
test refused_and_unsupported_orders ... ok
test refused_login_fails_the_connect ... ok
test recording_holds_the_frames_and_no_key ... ok
test reports_come_from_the_socket_and_rest ... ok
test reconnect_logs_in_again_and_catches_up ... ok
test stop_order_uses_the_stop_requests ... ok
test reports_follow_the_pages ... ok
test socket_state_follows_the_trade_socket ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s

     Running tests/python.rs (target/debug/deps/python-77b7db12ae4b2843)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests nautilus_qfex

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.32s
     Running unittests src/lib.rs (target/debug/deps/nautilus_qfex-1d94a765830d8d78)

running 155 tests
test book::tests::test_flags_and_empty_levels ... ok
test common::credential::tests::test_credential_env_vars ... ok
test book::tests::test_unsorted_input_is_ordered_and_bad_values_are_errors ... ok
test book::tests::test_each_message_replaces_the_book ... ok
test book::tests::test_rest_snapshot_depth_and_quote ... ok
test common::enums::tests::test_bar_interval::case_1 ... ok
test common::credential::tests::test_from_resolved_values ... ok
test common::enums::tests::test_bar_interval::case_2 ... ok
test common::enums::tests::test_bar_interval::case_3 ... ok
test common::credential::tests::test_from_resolved_values_rejects_partial_values ... ok
test common::credential::tests::test_debug_redacts_api_secret ... ok
test common::enums::tests::test_bar_interval::case_4 ... ok
test common::enums::tests::test_bar_interval::case_5 ... ok
test common::enums::tests::test_bar_interval::case_6 ... ok
test common::enums::tests::test_order_status_kind::case_02 ... ok
test common::enums::tests::test_order_status_kind::case_03 ... ok
test common::enums::tests::test_order_status_kind::case_04 ... ok
test common::enums::tests::test_order_status_kind::case_01 ... ok
test common::enums::tests::test_order_status_kind::case_06 ... ok
test common::enums::tests::test_order_status_kind::case_07 ... ok
test common::enums::tests::test_order_status_kind::case_08 ... ok
test common::enums::tests::test_order_status_kind::case_05 ... ok
test common::enums::tests::test_order_status_kind::case_10 ... ok
test common::enums::tests::test_order_status_kind::case_09 ... ok
test common::enums::tests::test_order_status_kind::case_11 ... ok
test common::enums::tests::test_side_accepts_both_cases::case_1 ... ok
test common::enums::tests::test_order_status_names_round_trip ... ok
test common::enums::tests::test_side_accepts_both_cases::case_2 ... ok
test common::parse::tests::test_precision_of::case_2 ... ok
test common::parse::tests::test_precision_of::case_1 ... ok
test common::parse::tests::test_precision_of::case_3 ... ok
test common::parse::tests::test_precision_of::case_4 ... ok
test common::parse::tests::test_precision_of_rejects_zero ... ok
test common::enums::tests::test_wire_names ... ok
test common::parse::tests::test_parse_rfc3339 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_2 ... ok
test common::parse::tests::test_symbol_round_trip ... ok
test common::parse::tests::test_unix_time_to_nanos::case_1 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_3 ... ok
test common::parse::tests::test_unix_time_to_nanos::case_4 ... ok
test common::parse::tests::test_unix_time_to_nanos_rejects_bad_values ... ok
test common::recording::tests::sent_auth_keeps_only_its_type ... ok
test common::recording::tests::other_frames_are_recorded_as_they_are ... ok
test common::recording::tests::rest_answer_carries_request_and_answer ... ok
test common::recording::tests::an_echoed_auth_loses_its_params_at_the_top_and_inside_err ... ok
test common::urls::tests::test_urls_per_environment::case_1 ... ok
test common::urls::tests::test_urls_per_environment::case_2 ... ok
test data::tests::deltas_and_depth_share_the_level2_channel ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_1 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_3 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_2 ... ok
test data::tests::next_bar_close_is_the_next_interval_boundary::case_4 ... ok
test config::exec::tests::debug_shows_the_record_path ... ok
test config::exec::tests::debug_hides_the_secret ... ok
test execution::order::tests::test_add_order_frames ... ok
test execution::order::tests::test_modify_and_cancel_frames ... ok
test execution::order::tests::test_stop_orders_carry_the_trigger_price::case_1 ... ok
test execution::order::tests::test_stop_orders_carry_the_trigger_price::case_2 ... ok
test execution::order::tests::test_what_the_venue_cannot_do_is_refused ... ok
test execution::dispatch::tests::test_rows_of_other_sessions_become_reports ... ok
test execution::tests::test_all_pages_stops_at_a_short_page_and_at_the_cap ... ok
test execution::dispatch::tests::test_refusals_reach_the_order_they_are_about ... ok
test execution::tests::test_only_later_pages_carry_an_offset ... ok
test execution::dispatch::tests::test_tracked_order_gets_events_through_a_modify_that_changes_its_id ... ok
test fuzzing::tests::test_book_apply_survives_any_levels ... ok
test fuzzing::tests::test_auth_survives_any_strings ... ok
test fuzzing::tests::test_wire_format_survives_25_decimals ... ok
test fuzzing::tests::test_wire_format_survives_any_decimals ... ok
test fuzzing::tests::test_mds_message_survives_mutated_frames ... ok
test http::client::tests::test_authenticated_request_without_credentials_is_not_sent ... ok
test http::client::tests::test_authenticated_request_is_signed ... ok
test http::client::tests::test_bars_and_funding_are_ascending_and_limited ... ok
test http::client::tests::test_error_statuses::case_1 ... ok
test http::client::tests::test_book_snapshot ... ok
test http::client::tests::test_error_statuses::case_2 ... ok
test http::client::tests::test_error_statuses::case_3 ... ok
test http::client::tests::test_iso_and_history_range ... ok
test http::client::tests::test_error_statuses::case_4 ... ok
test http::client::tests::test_other_authenticated_endpoints ... ok
test http::client::tests::test_path_segment::case_1 ... ok
test http::client::tests::test_path_segment::case_3 ... ok
test http::client::tests::test_path_segment::case_2 ... ok
test http::client::tests::test_path_segment::case_5 ... ok
test http::client::tests::test_path_segment::case_4 ... ok
test http::client::tests::test_path_segment::case_6 ... ok
test http::client::tests::test_public_request_sends_the_api_version_and_no_credentials ... ok
test http::error::tests::test_retry_after_only_on_http_and_rate_limit ... ok
test http::client::tests::test_unreadable_body_is_a_parse_error ... ok
test http::error::tests::test_should_retry::case_2 ... ok
test http::error::tests::test_should_retry::case_3 ... ok
test http::error::tests::test_should_retry::case_4 ... ok
test http::error::tests::test_should_retry::case_5 ... ok
test http::error::tests::test_should_retry::case_6 ... ok
test http::error::tests::test_should_retry::case_7 ... ok
test http::models::tests::test_authenticated_models ... ok
test http::error::tests::test_should_retry::case_1 ... ok
test http::models::tests::test_candles_and_funding ... ok
test http::models::tests::test_leverage_request_sends_a_number ... ok
test http::models::tests::test_order_book ... ok
test http::models::tests::test_refdata ... ok
test http::parse::tests::test_historic_order_status::case_1 ... ok
test http::parse::tests::test_historic_order_status::case_2 ... ok
test http::parse::tests::test_historic_order_status::case_3 ... ok
test http::parse::tests::test_historic_order_status::case_4 ... ok
test fuzzing::tests::test_trade_message_survives_mutated_frames ... ok
test http::parse::tests::test_parse_balance ... ok
test http::parse::tests::test_non_usd_quote_keeps_its_currency ... ok
test http::parse::tests::test_parse_bar ... ok
test http::parse::tests::test_parse_bar_widens_the_range ... ok
test http::parse::tests::test_parse_funding_rate ... ok
test http::parse::tests::test_parse_fill_report ... ok
test http::parse::tests::test_parse_historic_order_status_report ... ok
test http::parse::tests::test_parse_position_status_report ... ok
test signing::tests::test_default_source_gives_distinct_short_nonces_and_a_current_time ... ok
test signing::tests::test_headers ... ok
test signing::tests::test_hmac_primitive_matches_rfc_4231 ... ok
test http::parse::tests::test_parse_instrument_rejects_unusable_values ... ok
test http::parse::tests::test_parse_instruments ... ok
test signing::tests::test_nonce_length::case_1 ... ok
test signing::tests::test_nonce_length::case_2 ... ok
test signing::tests::test_nonce_length::case_4 ... ok
test signing::tests::test_signature_matches_openssl ... ok
test signing::tests::test_nonce_length::case_3 ... ok
test signing::tests::test_ws_message_has_integer_timestamp ... ok
test websocket::client::tests::mds_reconnect_request_follows_the_connection ... ok
test websocket::client::tests::mds_reconnect_resends_every_subscription_grouped_by_channel ... ok
test websocket::client::tests::mds_state_sink_sees_connect_loss_and_reconnect ... ok
test websocket::client::tests::mds_subscribe_batches_every_symbol_of_a_channel_into_one_frame ... ok
test websocket::client::tests::mds_subscribe_delivers_ack_then_data_and_answers_ping ... ok
test websocket::client::tests::send_without_a_connection_is_an_error ... ok
test websocket::client::tests::mds_subscribe_gate_holds_frames_past_the_inflight_cap ... ok
test websocket::client::tests::mds_subscribe_refused_by_the_venue_is_sent_again ... ok
test fuzzing::tests::test_rest_response_survives_mutated_bodies ... ok
test websocket::client::tests::trade_auth_for_a_connection_replaced_while_waiting_is_not_sent_and_the_next_logs_in ... ok
test websocket::handler::tests::test_captured_frames_give_every_data_type ... ok
test websocket::handler::tests::test_unknown_symbol_is_an_error_and_other_frames_give_nothing ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_1 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_2 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_3 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_4 ... ok
test websocket::messages::tests::test_bad_frames_are_errors::case_5 ... ok
test websocket::messages::tests::test_every_captured_mds_frame_parses ... ok
test websocket::messages::tests::test_every_trade_socket_frame_of_the_docs_parses ... ok
test websocket::messages::tests::test_failed_auth_is_an_error::case_1 ... ok
test websocket::messages::tests::test_failed_auth_is_an_error::case_2 ... ok
test websocket::messages::tests::test_login_answer_recorded_on_uat_is_authenticated ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_1 ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_2 ... ok
test websocket::messages::tests::test_login_answer_without_authenticated_true_is_an_error::case_3 ... ok
test websocket::messages::tests::test_mds_fields ... ok
test websocket::messages::tests::test_requests_match_the_docs ... ok
test websocket::messages::tests::test_trade_socket_fields ... ok
test websocket::client::tests::trade_socket_authenticates_then_starts_the_session_again_after_reconnect ... ok
test websocket::client::tests::trade_late_auth_answer_sends_nothing_on_the_next_socket_before_its_auth ... ok
test websocket::client::tests::trade_stale_reconnect_sends_one_auth_and_one_session_on_the_next_socket ... ok

test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s

     Running unittests bin/flatten.rs (target/debug/deps/qfex_flatten-59bc0a42074b7dd9)

running 8 tests
test tests::test_outcome_tells_a_refusal_from_progress::case_3 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_2 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_1 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_4 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_5 ... ok
test tests::test_outcome_tells_a_refusal_from_progress::case_6 ... ok
test tests::test_plan_cancels_first_then_closes_every_position ... ok
test tests::test_plan_sends_only_what_is_needed ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests bin/trades_probe.rs (target/debug/deps/qfex_trades_probe-8c7d118262a23c8f)

running 2 tests
test tests::test_arguments_give_the_venue_symbol_and_the_limit ... ok
test tests::test_trade_line_of_a_recorded_frame ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/data_client.rs (target/debug/deps/data_client-b422b590cc276b0d)

running 5 tests
test instrument_status_comes_from_refdata ... ok
test book_depth_is_ten_levels_of_the_pulsed_book ... ok
test connect_subscribe_reconnect_request_and_disconnect ... ok
test reconnect_command_reconnects_the_market_data_socket ... ok
test socket_state_follows_the_market_data_socket ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.08s

     Running tests/exec_client.rs (target/debug/deps/exec_client-0a0ca68a17fd7edd)

running 14 tests
test leverage_is_set_at_connect_and_on_request ... ok
test cancel_all_cancels_the_orders_of_the_strategy_one_by_one ... ok
test modify_follows_the_new_order_id ... ok
test cancel_all_without_a_side_uses_the_mass_cancel_when_it_hits_nothing_else ... ok
test connect_signs_rest_logs_in_and_subscribes ... ok
test order_is_accepted_filled_and_cancelled ... ok
test refused_and_unsupported_orders ... ok
test refused_login_fails_the_connect ... ok
test recording_holds_the_frames_and_no_key ... ok
test reports_come_from_the_socket_and_rest ... ok
test stop_order_uses_the_stop_requests ... ok
test reconnect_logs_in_again_and_catches_up ... ok
test reports_follow_the_pages ... ok
test socket_state_follows_the_trade_socket ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s

     Running tests/python.rs (target/debug/deps/python-ebef31c3f61f6618)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests examples/exec_tester.rs (target/debug/examples/exec_tester-8906594e7481d3ae)

running 20 tests
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_2 ... ok
test tests::each_step_drives_the_tester_the_way_the_gate_needs ... ok
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_3 ... ok
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_1 ... ok
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_5 ... ok
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_6 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_01 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_02 ... ok
test tests::every_step_runs_on_uat_and_records_to_its_own_file::case_4 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_04 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_05 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_06 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_07 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_08 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_03 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_10 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_09 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_11 ... ok
test tests::no_step_an_unknown_step_a_bad_instrument_or_mainnet_is_refused::case_12 ... ok
test tests::the_instrument_defaults_to_aapl_and_can_be_named_with_its_quantity ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests nautilus_qfex

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


==> test count
`cargo test --features examples`: 204 passed, as written
`cargo test`: 184 passed, as written

==> shortcut marker

all checks passed
```
