# U2 business rules to tests

Every business rule from BR-14 to BR-21 (`functional-design/business-rules.md`) and the tests that exercise it.
This extends the U1 table (`../../u1-foundation/code/br-test-traceability.md`), as MT-11 requires.

- Rust test names carry the rule ID (`br_18_4_...`).
- Where no test is named after a rule, the row lists the tests that cover it.
- Web test files are under `web/src`; end-to-end specs are under `web/e2e`.

Test layers:
- **unit**: core and the app over in-memory fakes, plus the adapters against loopback mock servers
- **postgres**: real PostgreSQL 18 over TLS
- **api**: the real router and pipeline
- **web**: Vitest and Testing Library, with axe
- **e2e**: Playwright in Chromium and Firefox, against the real stack and `e2e/mocks/server.mjs`

| Rule | Tests |
|---|---|
| BR-14.1 | All tests listed under BR-21.3 |
| BR-14.2 | `br_14_2_provider_names_are_trimmed_limited_and_compared_ignoring_case`, `br_14_2_names_are_unique_ignoring_case_and_every_field_problem_is_reported`, `br_14_2_a_provider_is_created_with_its_models_and_a_sealed_key`, postgres `provider_and_model_names_are_unique_and_the_database_enforces_it`; web `providers.test.tsx` (`checkProvider`, "refuses a bad form…", "shows a duplicate name from the server against the name field") |
| BR-14.3 | `br_14_3_model_names_and_lists_are_validated`, `br_14_3_every_field_problem_is_named_and_malformed_ids_are_not_found`, `model_labels_are_optional_and_limited`; web `checkProvider` (unique model names) |
| BR-14.4 | `br_20_3_an_empty_key_keeps_the_stored_one_set_replaces_it_and_remove_clears_it`, `br_20_3_a_key_can_be_kept_replaced_or_removed_explicitly`; web `providers.test.tsx` "edits a provider: an empty key keeps it…" and "removes the saved key only when asked"; e2e `integrations.spec.ts` "a provider key is never shown again…" |
| BR-14.5 | `br_14_5_a_test_replaces_detected_values_and_never_a_declared_one`, `br_14_5_editing_models_keeps_detected_capabilities_and_applies_declared_ones`, `undeclared_values_follow_detection_and_agreement_is_not_a_disagreement`, postgres `providers_and_models_round_trip_keep_detected_values_and_cascade_on_delete` |
| BR-14.6 | Adapter: `a_capable_model_is_detected_with_tool_calling_and_its_context_length`, `each_context_field_a_provider_may_use_is_understood`, `a_model_that_answers_in_plain_text_has_no_tool_calling`, `a_provider_that_rejects_tools_means_unsupported`, `an_inconclusive_tool_probe_is_unknown`, `a_refused_key_stops_the_test_before_any_model_is_called`, `an_unknown_model_is_reported_and_the_provider_still_counts_as_working`, `an_answer_that_is_not_a_chat_completion_is_a_bad_response`, `a_provider_that_cannot_be_reached_fails_the_provider_and_every_model`, `test_statuses_round_trip_and_have_fixed_messages`; postgres `a_started_test_runs_once_across_two_workers_and_stores_its_results`; api `a_provider_test_runs_end_to_end_over_postgres_with_the_real_adapter_and_leaks_nothing`; e2e "add a provider, test it, read its capabilities…" |
| BR-14.7 | `a_slow_model_times_out_when_the_budget_runs_out_but_finished_ones_are_kept`, `a_slow_answer_times_out`, `models_are_probed_four_at_a_time_and_all_of_them_finish`, api `rl_8_a_test_is_accepted_runs_in_the_background_and_a_second_start_answers_409_with_its_id`, postgres `only_one_run_per_target_can_be_active_and_the_old_ones_are_pruned`, `two_workers_cannot_hold_the_same_target_lock_and_a_dropped_guard_frees_it`, `br_21_3_starting_and_reading_tests_is_for_admins_and_needs_a_real_provider`, api `se_22_test_results_never_contain_remote_text` |
| BR-14.8 | `br_14_8_a_model_without_what_a_job_needs_is_flagged`; web `CapabilityWarning` tests, `DefaultModelsCard` "warns beside an unsuitable pick…", the provider test panel's warnings. The runtime refusal itself is U4 |
| BR-15.1 | `br_15_1_a_default_must_be_a_listed_model`; web `DefaultModelsCard` "names the provider of each default and saves by model id" |
| BR-15.2 | `br_15_2_suitability_needs_tool_calling_and_enough_context`, `br_15_2_an_unsuitable_default_is_allowed_with_warnings_and_unknown_models_are_refused`; web "warns beside an unsuitable pick and needs \"Use anyway\""; e2e "add a provider…" (defaults with an acknowledged warning) |
| BR-15.3 | `br_15_3_deleting_a_model_or_provider_clears_defaults_that_point_at_it`, `br_15_3_removing_a_model_clears_a_default_that_pointed_at_it`, `br_15_3_removing_models_clears_the_defaults_that_pointed_at_them`, `br_15_3_and_w18_removing_a_provider_reports_then_clears_what_depends_on_it`, api `w18_removing_a_provider_reports_what_it_clears_and_clears_it`, postgres `deleting_a_provider_through_the_service_clears_its_defaults_in_one_transaction`; web "removes a provider after naming the defaults it clears"; e2e "add a provider…" (removal names both defaults) |
| BR-15.4 | Defaults are read only when a session is created (U3 creates sessions); `br_15_1_a_default_must_be_a_listed_model` shows a change writes only the defaults record. The screen states it (web `defaultsIntro`) |
| BR-15.5 | web "names the provider of each default…" (`From Colibri`, options `{provider} / {model}`); e2e "add a provider…" (`From <name>` after reload) |
| BR-16.1 | `br_16_1_one_search_provider_is_active_and_keys_are_needed_before_activation`, api `w20_search_providers_are_saved_tested_and_one_is_activated`; web `search-github.test.tsx` "shows three kinds and says search is not set up…", "turns search off"; e2e "switch the search provider and test it" |
| BR-16.2 | `br_16_2_searxng_defaults_to_the_bundled_service_and_other_kinds_take_no_url_or_wrong_key`, `br_16_search_kinds_know_what_they_need`, `the_real_endpoints_are_the_fixed_public_ones`; web `checkSearchKind`, "sends an empty SearXNG URL as \"use the bundled service\"…" |
| BR-16.3 | postgres `search_connector_and_policy_round_trip`, `br_20_1_sealed_secrets_round_trip_through_the_database_bound_to_their_row`; api `w20_…` (settings of inactive kinds kept) |
| BR-16.4 | `searxng_asks_for_json_and_succeeds_on_results`, `no_results_is_a_success_with_a_note`, `a_refused_key_is_unauthorised_for_the_providers_that_use_keys`, `other_failures_and_odd_answers_are_bad_responses`, `an_unreachable_server_is_reported_as_such`, `searxng_with_json_switched_off_is_a_bad_response_not_a_key_problem`; web "warns when activating a provider that never passed a test", "saves a Brave key…, then tests and activates it" (no-results note) |
| BR-16.5 | Delivered in U4 (citations in threads) |
| BR-17.1 | `the_real_api_base_is_fixed` |
| BR-17.2 | `br_17_2_the_github_token_is_sealed_replaced_and_removed`, `br_20_2_no_response_ever_carries_a_secret`; web "saves a token without keeping it…"; e2e "save and remove the GitHub token" (absent from the page after reload) |
| BR-17.3 | `br_17_3_only_the_first_hundred_repositories_are_kept_with_the_true_total`, `a_good_token_reports_the_account_the_expiry_and_the_repositories`, `many_repositories_are_counted_from_the_last_page_and_only_a_hundred_are_listed`, `a_refused_token_is_unauthorised_and_nothing_else_is_asked`, `answers_that_are_not_what_github_sends_are_bad_responses`, `an_unreachable_github_is_reported_as_such`; web "tests the connection and shows the account and repositories", `TestResultView` "lists GitHub repositories with the total when capped" |
| BR-17.4 | web `search-github.test.tsx` "always shows the minimum permissions and the status"; e2e "save and remove the GitHub token" |
| BR-17.5 | Structural: GitHub is reached only through the `Connector` port (`crates/github` implements it); api `w21_the_github_token_is_saved_tested_and_removed` |
| BR-17.6 | api `w21_…` (removal turns the connector off); web "removes the token after saying what stops working"; e2e "save and remove the GitHub token" |
| BR-18.1 | `br_18_1_an_instance_that_never_saved_a_policy_has_the_factory_default`; web `sandbox.test.tsx` (the "Changes apply to runs that start after you save" banner) |
| BR-18.2 | `br_18_2_the_container_tier_needs_a_runtime_that_answers_a_fresh_probe`, `br_18_2_the_container_tier_is_refused_until_the_runtime_answers`, `no_socket_configured_is_not_configured`, `a_socket_that_does_not_exist_is_unreachable`, api `w23_a_runtime_check_is_a_test_run`; web "offers the container runner only after a successful runtime check", "keeps the container runner unavailable…"; e2e "edit the sandbox policy… container tier unavailable" |
| BR-18.3 | `br_18_3_a_policy_with_command_run_is_refused`, `br_18_3_and_18_4_an_invalid_policy_reports_every_problem_by_field`; web (the `command_run` checkbox is disabled with its reason; a server refusal shows beside the tools) |
| BR-18.4 | `br_18_4_host_names_are_normalised_and_checked`, `br_18_4_an_allow_list_is_unique_lower_case_limited_and_reports_every_bad_entry`; web `checkHosts`, `HostListEditor`, "refuses out-of-range limits and bad hosts…" |
| BR-18.5 | `br_18_5_every_limit_can_be_set_anywhere_in_its_range_and_not_outside`, `br_18_5_a_saved_limit_wins_and_each_save_raises_the_version`; web "resets a limit to its factory default and saves the whole policy"; e2e "edit the sandbox policy, reset a limit…" |
| BR-18.6 | Structural: `PolicyRequest` has no path field and request bodies deny unknown fields (api `w22_the_sandbox_policy_is_read_with_its_ranges_validated_and_saved`) |
| BR-18.7 | `br_21_2_every_change_and_test_is_audited_with_before_and_after_and_no_secret` (policy before and after) |
| BR-18.8 | `br_18_8_the_effective_policy_is_the_instance_policy_or_a_session_policy_tightened_to_it`, `br_18_8_a_session_may_tighten_each_setting`, `br_18_8_a_session_cannot_exceed_the_instance_policy_on_any_setting`, `br_18_8_a_session_policy_equal_to_the_instance_policy_is_within_it`, `br_18_8_clamping_applies_a_lowered_instance_policy_to_an_existing_session_value`, `br_18_8_clamping_to_no_network_removes_all_hosts_and_never_loosens`, `br_18_8_network_rules_compare_by_coverage` |
| BR-19.1 | `br_19_1_accepts_ordinary_https_and_local_http_urls_and_normalises_them`, `br_19_1_refuses_malformed_and_unsafe_shapes`, `a_request_path_cannot_change_the_host_the_url_was_validated_for`; web `checkOutboundUrl` |
| BR-19.2 | `br_19_2_http_is_only_for_local_and_private_hosts`, `private_and_loopback_names_are_allowed`; web `checkOutboundUrl`; e2e "a provider key is never shown again and a bad URL is refused…" |
| BR-19.3 | `br_19_3_link_local_and_metadata_addresses_are_refused_in_every_spelling_and_scheme`, `br_19_3_unusable_addresses_are_refused`, `only_link_local_metadata_and_unusable_addresses_are_always_blocked`; web `checkOutboundUrl`, "refuses a bad form…" |
| BR-19.4 | `a_name_is_checked_each_time_so_rebinding_to_a_blocked_address_is_refused`, `a_name_that_does_not_resolve_is_unreachable` |
| BR-19.5 | `a_redirect_on_the_same_scheme_host_and_port_is_followed`, `a_redirect_to_another_host_or_port_is_refused`, `a_scheme_change_or_a_loop_or_a_redirect_without_a_location_is_refused`, `at_most_three_redirects_are_followed`, `a_307_keeps_the_method_and_body_and_a_302_turns_a_post_into_a_get` |
| BR-19.6 | `a_response_over_the_cap_is_too_large_whether_declared_or_streamed`, `an_oversized_answer_is_a_bad_response`, `https_to_a_server_that_does_not_speak_tls_is_unreachable_never_downgraded` |
| BR-19.7 | `br_19_7_url_refusals_come_back_with_the_reason_and_the_field`, `every_error_maps_to_a_fixed_test_status`; web "sends an empty SearXNG URL… shows URL refusals", `UrlField` |
| BR-19.8 | Could-have (US-2.12), not built; there is deliberately no setting that relaxes BR-19.3 |
| BR-20.1 | `br_20_1_sealed_secrets_round_trip_through_the_database_bound_to_their_row`, `br_20_1_the_runner_opens_the_key_for_the_probe_and_nothing_else_keeps_it` |
| BR-20.2 | `br_20_2_no_response_ever_carries_a_secret`, `br_20_2_and_20_4_no_view_or_audit_entry_carries_the_key`; web `SecretField` tests |
| BR-20.3 | `br_20_3_an_empty_key_keeps_the_stored_one_set_replaces_it_and_remove_clears_it`, `br_20_3_a_key_can_be_kept_replaced_or_removed_explicitly`; web "adds a provider with models, sending the key once and never keeping it" (the key is not in the DOM or in storage afterwards), "saves a Brave key without keeping it…", "saves a token without keeping it…"; e2e "a provider key is never shown again…" |
| BR-20.4 | `br_20_2_and_20_4_no_view_or_audit_entry_carries_the_key`, `the_key_is_sent_as_a_bearer_token_and_appears_in_no_result`, api `a_provider_test_runs_end_to_end_over_postgres_with_the_real_adapter_and_leaks_nothing` (responses, audit entries, captured log lines), `calls_are_counted_by_target_kind_and_outcome_never_by_host` |
| BR-21.1 | `br_21_1_if_the_audit_write_fails_the_change_does_not_happen` |
| BR-21.2 | `br_21_2_every_change_and_test_is_audited_with_before_and_after_and_no_secret` |
| BR-21.3 | `br_21_3_managing_integrations_is_for_admins_with_a_full_scope_sign_in_only`, `br_21_3_only_signed_in_admins_reach_any_of_it`, `br_21_3_anonymous_callers_are_unauthenticated_and_members_are_told_the_route_is_absent`, `br_21_3_starting_and_reading_tests_is_for_admins_and_needs_a_real_provider`, api `an_admin_who_has_not_finished_mfa_enrolment_cannot_reach_them`, `se_9_changes_need_the_csrf_token`; web "shows \"not found\" to members, and hides the link"; e2e "the integrations pages are \"not found\" for members, and the API agrees" |
| BR-21.4 | The config read port (PR 3: a 5 s cached server-side read of providers, defaults, search, connector and policy); `br_20_1_the_runner_opens_the_key_for_the_probe_and_nothing_else_keeps_it` shows that only the server opens secrets |

## Stories

| Story | Web and end-to-end coverage |
|---|---|
| US-2.1 Configure an OpenAI-compatible provider | web `providers.test.tsx`; e2e "add a provider…", "a provider key is never shown again…" |
| US-2.3 Test a provider connection | web `providers.test.tsx` (background test, per-model results); `components/__tests__/integrations.test.tsx` (`useTestRun`: polling, 409 follow, time limit, rate limit); e2e "add a provider…" |
| US-2.4 Set default models | web `DefaultModelsCard` tests; e2e "add a provider…" |
| US-2.5 Configure web search | web `search-github.test.tsx`; e2e "switch the search provider and test it" |
| US-2.6 Configure the GitHub connector | web `search-github.test.tsx`; e2e "save and remove the GitHub token" |
| US-2.9 Configure the sandbox policy | web `sandbox.test.tsx`; e2e "edit the sandbox policy…" |
| US-5 U2 screens meet the U1 baseline | web `__tests__/accessibility.test.tsx` and the axe check in every page test; e2e `accessibility.spec.ts` (four U2 pages in both themes and at 360 px) and "the dialogs have no accessibility violations in either theme" |
