# U1 business rules to tests

Every business rule BR-1 to BR-13 (`functional-design/business-rules.md`) and the tests that exercise it.
Rust test names carry the rule ID (`br_4_3_...`); a name with two IDs covers both. Rows without a named test list the
covering tests directly.

Test layers: **unit** (core, app over in-memory fakes), **postgres** (real PostgreSQL 18 over TLS),
**api** (real router and pipeline), **web** (Vitest and Testing Library with axe), **e2e** (Playwright, Chromium and Firefox).

| Rule | Tests |
|---|---|
| BR-1.1 | `br_1_1_first_run_setup_exists_only_for_anonymous_callers_while_no_user_exists`, `br_1_1_setup_creates_an_admin_with_an_enrolment_only_sign_in_and_audits_it`, `br_1_1_setup_disappears_once_a_user_exists`, `br_1_1_two_simultaneous_setups_create_exactly_one_admin` |
| BR-1.2 | `br_1_2_a_member_signs_in_with_a_full_scope_and_it_is_audited`, `br_1_2_case_variants_are_the_same_identifier`, `br_1_2_email_is_trimmed_and_lower_cased`, `br_1_2_enforces_length_limits`, `br_1_2_rejects_malformed_addresses` |
| BR-1.3 | `br_1_3_rejects_empty_long_and_control_characters`, `br_1_3_trims_and_accepts_one_to_eighty_characters` |
| BR-1.4 | `br_1_4_a_deactivated_account_is_indistinguishable_from_a_wrong_password`, `br_1_4_a_deactivated_user_cannot_sign_in_and_reactivation_restores_it`, `br_1_4_and_7_4_deactivation_blocks_sign_in_and_reactivation_restores_it` |
| BR-1.5 | `br_1_5_creation_paths_are_only_setup_and_invitation` |
| BR-2.1 | `br_2_1_length_counts_characters_not_bytes`, `br_2_1_length_is_ten_to_one_hundred_twenty_eight_characters`, `br_2_1_short_plain_passwords_can_be_left_out_because_the_policy_refuses_them_anyway` |
| BR-2.2 | `br_2_2_a_space_counts_as_a_symbol`, `br_2_2_requires_each_character_kind` |
| BR-2.3 | `br_2_3_a_breached_password_is_refused`, `br_2_3_a_hash_corpus_becomes_a_filter_that_finds_each_listed_password`, `br_2_3_corrupt_or_foreign_filter_files_are_rejected`, `br_2_3_listed_passwords_are_found_and_others_are_not`, `br_2_3_the_embedded_filter_knows_common_breached_passwords_and_not_a_fresh_passphrase`, `br_2_3_the_false_positive_rate_matches_the_design`, `br_2_3_the_filter_round_trips_through_its_file_form`, `br_2_3_the_limit_keeps_only_the_most_prevalent_entries`, `br_2_3_the_online_check_is_used_only_when_asked_and_sends_only_a_five_character_prefix`, `br_2_4_and_2_3_violations_have_plain_requirements` |
| BR-2.4 | `br_2_4_and_2_3_violations_have_plain_requirements`, `br_2_4_the_new_password_must_differ_and_meet_the_policy` |
| BR-2.5 | `br_2_5_the_same_password_policy_applies_at_setup_invitation_change_and_reset` |
| BR-2.6 | `br_2_6_reports_every_unmet_requirement_without_echoing_the_password`, `br_2_6_setup_reports_every_problem_without_echoing_the_password` |
| BR-3.1 | `br_3_1_a_failure_delays_the_next_attempt_and_the_delay_ends`, `br_3_1_attempts_before_the_delay_ends_are_refused`, `br_3_1_delay_doubles_and_caps_at_60s` |
| BR-3.2 | `br_3_2_failures_during_a_lock_keep_it_locked`, `br_3_2_ten_failures_in_an_hour_lock_the_account_for_fifteen_minutes`, `br_3_2_the_tenth_failure_in_an_hour_locks_for_fifteen_minutes` |
| BR-3.3 | `br_3_3_success_resets_everything`, `br_3_3_success_resets_the_failure_count`, `br_3_3_the_window_resets_after_an_hour_without_failures` |
| BR-3.4 | api (`apps/api/tests/pipeline.rs`): `sign_in_is_limited_per_source_with_retry_after`, `forwarded_headers_cannot_dodge_the_limit_unless_the_peer_is_a_trusted_proxy`, `first_run_setup_has_the_tightest_limit` |
| BR-3.5 | `br_3_5_a_wrong_password_and_an_unknown_email_get_the_same_answer`, `br_3_5_the_dummy_verification_does_the_same_work_without_matching`, `br_3_5_unknown_email_guards_round_trip_and_the_table_stays_bounded`, `br_3_5_unknown_emails_are_throttled_exactly_like_known_ones` |
| BR-3.6 | `br_3_6_a_refused_attempt_does_not_check_the_password` |
| BR-3.7 | `br_3_7_and_6_4_there_is_no_unlock_and_no_self_service_forgot_password` |
| BR-4.1 | `br_4_1_a_member_promoted_to_admin_is_limited_to_enrolment_until_they_enrol`, `br_4_1_an_admin_without_mfa_gets_an_enrolment_only_sign_in`, `br_4_1_an_enrolment_only_session_can_only_manage_mfa_sign_out_and_look_itself_up`, `br_4_1_an_enrolment_only_sign_in_cannot_do_anything_else`, `br_4_1_an_invited_admin_starts_with_an_enrolment_only_sign_in`, `br_4_1_confirming_enrolment_upgrades_an_enrolment_only_sign_in`, `br_4_1_enrolment_completes_once`, `br_4_1_enrolment_only_sessions_upgrade_to_full` |
| BR-4.2 | `br_4_2_mfa_is_optional_for_members_and_they_can_enrol_at_any_time` |
| BR-4.3 | `br_4_3_a_code_is_accepted_one_step_either_side_but_not_two`, `br_4_3_and_4_8_a_correct_code_signs_in_and_a_wrong_code_counts_as_a_failure`, `br_4_3_matches_the_rfc_6238_test_vectors`, `br_4_3_with_mfa_the_password_alone_asks_for_a_second_factor_without_counting_a_failure` |
| BR-4.4 | `br_4_4_a_time_step_cannot_be_reused_or_go_backwards`, `br_4_4_a_totp_code_cannot_be_replayed`, `br_4_4_the_matching_step_is_reported_so_replays_can_be_refused` |
| BR-4.5 | `br_4_5_a_recovery_code_is_single_use`, `br_4_5_a_recovery_code_signs_in_once_and_is_audited`, `br_4_5_a_set_has_ten_codes_and_a_reminder_shows_at_three_or_fewer`, `br_4_5_enrolment_yields_ten_distinct_recovery_codes_and_is_audited`, `br_4_5_recovery_code_matching_ignores_case_dashes_and_spaces`, `br_4_5_recovery_codes_are_unique_across_a_set_of_ten`, `br_4_5_recovery_codes_use_the_safe_alphabet_and_format`, `br_4_5_regenerating_replaces_every_earlier_code` |
| BR-4.6 | `br_4_6_a_member_can_switch_mfa_off_with_password_and_a_current_code`, `br_4_6_an_admin_cannot_switch_mfa_off` |
| BR-4.7 | `br_4_7_the_cli_issues_a_reset_for_the_last_admin_and_audits_it`, `br_4_7_the_cli_refuses_when_there_are_several_admins_or_the_user_is_not_an_admin` |
| BR-4.8 | `br_4_3_and_4_8_a_correct_code_signs_in_and_a_wrong_code_counts_as_a_failure` |
| BR-5.1 | `br_5_1_an_invitation_returns_a_one_time_link_token_and_stores_only_its_hash`, `br_5_1_duplicate_emails_are_refused` |
| BR-5.2 | `br_5_2_an_invitation_expires_after_seven_days`, `br_5_2_invitations_last_seven_days_and_br_6_1_resets_last_a_day` |
| BR-5.3 | `br_5_3_an_invitation_link_is_shown_once_and_cannot_be_listed_again` |
| BR-5.4 | `br_5_4_an_expired_invitation_cannot_be_accepted`, `br_5_4_unknown_tokens_and_weak_passwords_are_handled_without_leaking` |
| BR-5.5 | `br_5_5_acceptance_is_single_use`, `br_5_5_accepting_creates_the_user_with_the_invited_role_and_is_single_use` |
| BR-5.6 | `br_5_6_a_revoked_invitation_acts_like_an_invalid_link`, `br_5_6_a_revoked_invitation_behaves_like_an_invalid_link` |
| BR-6.1 | `br_6_1_a_reset_link_expires_after_24_hours`, `br_6_1_a_reset_link_is_single_use`, `br_6_1_a_reset_link_works_for_24_hours` |
| BR-6.2 | `br_6_2_unknown_links_get_the_generic_answer` |
| BR-6.3 | `br_6_3_issuing_a_new_link_invalidates_earlier_unused_ones`, `br_6_3_issuing_a_new_link_invalidates_the_earlier_one` |
| BR-6.4 | `br_3_7_and_6_4_there_is_no_unlock_and_no_self_service_forgot_password` |
| BR-6.5 | `br_6_5_also_reset_mfa_clears_it_and_an_admin_must_enrol_again` |
| BR-7.1 | `br_7_1_roles_change_between_admin_and_member_only`, `br_7_1_roles_change_with_old_and_new_values_audited` |
| BR-7.2 | `br_7_2_members_and_inactive_admins_are_not_protected`, `br_7_2_the_last_active_admin_cannot_be_deactivated_or_demoted`, `br_7_2_two_admins_demoting_each_other_at_once_leave_one_admin` |
| BR-7.3 | `br_7_3_an_admin_cannot_deactivate_themselves`, `br_7_3_nobody_deactivates_themselves` |
| BR-7.4 | `br_1_4_and_7_4_deactivation_blocks_sign_in_and_reactivation_restores_it`, `br_7_4_reactivation_keeps_the_password` |
| BR-7.5 | e2e `deactivation.spec.ts` (the list shows the person as Deactivated); web `pages/admin/__tests__/users.test.tsx` |
| BR-7.6 | web `ConfirmDialog` test "can require typing the exact text before confirming" and `users.test.tsx`; e2e `deactivation.spec.ts` (type the email to confirm) |
| BR-8.1 | `br_8_1_sign_out_ends_only_that_sign_in_and_is_audited`, `br_8_1_tokens_are_256_bit_and_only_the_hash_is_derivable_from_them` |
| BR-8.2 | `br_8_2_absolute_duration_applies_regardless_of_activity`, `br_8_2_an_active_sign_in_ends_at_the_absolute_duration`, `br_8_2_an_idle_sign_in_ends_after_the_idle_timeout`, `br_8_2_changed_durations_apply_only_to_new_sign_ins`, `br_8_2_idle_timeout_applies_from_last_use`, `br_8_2_the_limits_are_those_stored_on_the_session` |
| BR-8.3 | `br_8_3_an_ended_session_stays_ended_with_its_first_reason`, `br_8_3_changing_the_password_ends_other_sign_ins_but_not_this_one` |
| BR-8.4 | `br_9_1_and_8_4_deactivation_takes_effect_on_the_very_next_request` |
| BR-8.5 | `br_8_5_a_person_can_hold_several_sessions_at_once` |
| BR-9.1 | `br_9_1_a_demotion_changes_the_role_on_the_next_request`, `br_9_1_and_8_4_deactivation_takes_effect_on_the_very_next_request` |
| BR-9.2 | `br_9_2_admin_resources_are_not_found_for_members`, `br_9_2_members_cannot_invite_and_the_denial_is_audited`, `br_9_2_members_cannot_see_or_change_settings_or_the_audit_log`, `br_9_2_only_admins_can_issue_reset_links` |
| BR-9.3 | `br_9_3_an_unauthenticated_caller_cannot_use_a_signed_in_action`, `br_9_3_members_can_change_their_password_and_use_chat`, `br_9_3_public_entry_points_are_open_to_anyone`, `br_9_3_signed_in_actions_need_a_session` |
| BR-10.1 | `br_10_1_absolute_range_is_1_hour_to_90_days`, `br_10_1_idle_timeout_range_is_15_minutes_to_30_days`, `br_10_1_out_of_range_values_are_refused_and_an_unchanged_save_audits_nothing`, `br_10_1_the_idle_timeout_cannot_exceed_the_absolute_duration` |
| BR-10.2 | `br_10_2_defaults_apply_until_something_is_stored`, `br_10_2_defaults_are_12_hours_14_days_and_online_check_off` |
| BR-10.3 | `br_10_3_each_changed_value_is_audited_with_old_and_new` |
| BR-11.1 | `br_11_1_if_the_audit_entry_cannot_be_written_the_change_does_not_happen` |
| BR-11.2 | postgres: `se_15_the_app_role_can_append_and_read_audit_entries_but_never_change_or_delete_them`, `cleanup_removes_expired_rows_and_never_touches_the_audit_log`; cli: `audit_prune_needs_a_privileged_role_a_safe_cutoff_and_confirmation`; api: `postgres_flow`, `app_flow` |
| BR-11.3 | `br_11_3_failures_are_written_only_for_existing_accounts` |
| BR-11.4 | `br_11_4_details_that_look_like_secrets_are_dropped`, `br_11_4_no_secret_ever_appears_in_the_audit_log` |
| BR-11.5 | `br_11_5_the_catalogue_has_twenty_unique_dotted_names_that_round_trip` |
| BR-12.1 | e2e `accessibility.spec.ts` ("fits a 360 px screen without sideways scrolling"), `keyboard.spec.ts` (narrow-screen menu); web `shell.test.tsx` (menu toggle and class), `ResponsiveTable` tests |
| BR-12.2 | web `shell.test.tsx` ("remembers the theme choice on the root element"); e2e `keyboard.spec.ts` ("the theme choice sticks across a reload"), `accessibility.spec.ts` (every page in light and dark) |
| BR-12.3 | e2e `accessibility.spec.ts` (axe, WCAG 2.2 AA tags, every page, both themes), `keyboard.spec.ts` (keyboard operation, focus, skip link, dialogs); web axe check on every page, dialog and shared component |
| BR-12.4 | web `public-setup-signin.test.tsx` and `FormField` tests (errors beside fields, linked and not colour-only, passwords not kept); e2e `keyboard.spec.ts` ("an empty form moves focus to the first problem") |
| BR-12.5 | web `CopyOnceSecret` tests (warning, copy, acknowledge to dismiss); e2e `invitation.spec.ts`, `reset.spec.ts`, `setup.spec.ts` (links and recovery codes); api: `br_5_3_an_invitation_link_is_shown_once_and_cannot_be_listed_again` |
| BR-13.1 | `br_13_1_a_skeleton_session_is_visible_only_to_its_owner`, `br_13_1_and_13_2_titles_and_bodies_are_validated`, `br_13_1_the_event_feed_shows_only_the_owners_events_and_flags_old_positions`, `br_13_1_titles_are_one_to_one_hundred_twenty_characters` |
| BR-13.2 | `br_13_1_and_13_2_titles_and_bodies_are_validated`, `br_13_2_message_bodies_are_one_to_four_thousand_characters` |
| BR-13.3 | `br_13_3_a_crashed_worker_lease_lapses_and_counts_as_a_failed_attempt`, `br_13_3_a_crashed_workers_lapsed_lease_is_recovered_and_retried`, `br_13_3_a_failing_job_retries_after_5s_30s_and_2min_then_fails`, `br_13_3_a_failing_provider_retries_after_5s_30s_2min_then_fails_without_losing_the_message` |
| BR-13.4 | `br_13_4_duplicate_delivery_never_produces_a_second_reply`, `br_13_4_outbox_events_are_ordered_notify_on_commit_and_can_be_replayed_after_a_position`, `br_13_4_posting_writes_the_message_and_an_outbox_event_together` |
| BR-13.5 | `br_13_5_the_default_reply_echoes_the_first_200_characters`, `br_13_5_the_same_input_always_gives_the_same_reply` |

## Stories to end-to-end scenarios

| Story | End-to-end scenario (`web/e2e`) |
|---|---|
| US-1.1 First-run setup | `setup.spec.ts`: setup, forced MFA enrolment, recovery codes, setup closed afterwards |
| US-1.2 Sign in | `invitation.spec.ts`, `reset.spec.ts`, `deactivation.spec.ts`: sign-in with email and password; `keyboard.spec.ts` |
| US-1.3 Invite users | `invitation.spec.ts`: invite, copy once, accept, link works once |
| US-1.4 Deactivate and reactivate | `deactivation.spec.ts`: typed confirmation, immediate sign-out, reactivation |
| US-1.5 MFA | `setup.spec.ts`: enrolment; `roles.spec.ts`: a promoted admin must enrol |
| US-1.6 Change password | web `public-mfa-account.test.tsx`; api `br_2_5_...` |
| US-1.7 Forced reset | `reset.spec.ts`: issue, sign-out everywhere, one use, old password refused |
| US-1.8 Change role | `roles.spec.ts`: promote, demote, effect on the next page |
| US-1.9 Sign-in duration and breach check | web `pages/admin/__tests__/settings.test.tsx`; app `br_10_*` |
| US-2.7 Audit log | web `audit.test.tsx`; api `postgres_flow` (filters, paging, newest first) |
| US-13.3 Sign-in protection | `throttling.spec.ts`: countdown, disabled button, one wording; api `pipeline.rs` rate-limit and lockout tests |
| US-13.7 Mobile, themes, WCAG baseline | `accessibility.spec.ts`, `keyboard.spec.ts` |
| Skeleton | `skeleton.spec.ts`: message to agent reply over the WebSocket; chats are private |
