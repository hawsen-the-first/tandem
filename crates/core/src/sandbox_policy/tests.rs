use super::*;

fn hosts(entries: &[&str]) -> NetworkRule {
    NetworkRule::allow_hosts(entries.iter().copied()).unwrap()
}

fn policy() -> SandboxPolicy {
    SandboxPolicy::factory_default()
}

// ---- the registry (A1, BR-18.3) -----------------------------------------------------------------

#[test]
fn a1_the_registry_lists_six_tools_and_command_run_can_never_be_enabled() {
    assert_eq!(ToolId::ALL.len(), 6);
    for tool in ToolId::ALL {
        assert_eq!(ToolId::parse(tool.as_str()), Some(tool));
        assert!(!tool.description().is_empty());
    }
    assert_eq!(ToolId::parse("shell"), None);
    assert!(!ToolId::CommandRun.can_be_enabled());
    assert!(
        ToolId::ALL
            .into_iter()
            .filter(|tool| tool.can_be_enabled())
            .count()
            == 5
    );
}

#[test]
fn br_18_3_a_policy_with_command_run_is_refused() {
    let mut policy = policy();
    policy.tool_allow_list.insert(ToolId::CommandRun);
    let issues = policy.validate().unwrap_err();
    assert_eq!(issues, vec![PolicyIssue::CommandRunNotAllowed]);
    assert_eq!(issues[0].field(), "tool_allow_list");
}

#[test]
fn the_factory_default_is_valid_and_conservative() {
    let policy = policy();
    assert!(policy.validate().is_ok());
    assert_eq!(policy.scope, PolicyScope::Instance);
    assert_eq!(policy.runner_tier, RunnerTier::InProcess);
    assert_eq!(policy.network, NetworkRule::None, "default deny (FR-16.1)");
    assert!(!policy.tool_allow_list.contains(&ToolId::CommandRun));
    assert_eq!(policy.tool_allow_list.len(), 5);
    assert_eq!(policy.version, 1);
}

// ---- limits and ranges (A2, BR-18.5) ------------------------------------------------------------

#[test]
fn a2_the_factory_limits_are_the_documented_values_and_inside_their_ranges() {
    let limits = Limits::factory();
    assert_eq!(limits.wall_clock_seconds, 600);
    assert_eq!(limits.max_output_bytes, 1_048_576);
    assert_eq!(limits.max_file_bytes, 5 * 1_048_576);
    assert_eq!(limits.max_files_written, 200);
    assert_eq!(limits.memory_mebibytes, 512);
    assert_eq!(limits.cpu_millicores, 1_000);
    for (name, value, range) in limits.fields() {
        assert!(range.contains(value), "{name}");
        assert_eq!(range.default, value, "{name}");
        assert!(range.min < range.max, "{name}");
    }
}

#[test]
fn br_18_5_every_limit_can_be_set_anywhere_in_its_range_and_not_outside() {
    for (field, range) in [
        ("wall_clock_seconds", WALL_CLOCK_SECONDS),
        ("max_output_bytes", MAX_OUTPUT_BYTES),
        ("max_file_bytes", MAX_FILE_BYTES),
        ("max_files_written", MAX_FILES_WRITTEN),
        ("memory_mebibytes", MEMORY_MEBIBYTES),
        ("cpu_millicores", CPU_MILLICORES),
    ] {
        let set = |value: u64| {
            let mut policy = policy();
            match field {
                "wall_clock_seconds" => policy.limits.wall_clock_seconds = value,
                "max_output_bytes" => policy.limits.max_output_bytes = value,
                "max_file_bytes" => policy.limits.max_file_bytes = value,
                "max_files_written" => policy.limits.max_files_written = value,
                "memory_mebibytes" => policy.limits.memory_mebibytes = value,
                _ => policy.limits.cpu_millicores = value,
            }
            policy
        };
        assert!(set(range.min).validate().is_ok(), "{field} at its minimum");
        assert!(set(range.max).validate().is_ok(), "{field} at its maximum");
        let below = set(range.min - 1).validate().unwrap_err();
        assert_eq!(
            below,
            vec![PolicyIssue::OutOfRange {
                field,
                min: range.min,
                max: range.max
            }]
        );
        assert_eq!(below[0].field(), field);
        assert!(
            set(range.max + 1).validate().is_err(),
            "{field} above its maximum"
        );
    }
}

#[test]
fn every_problem_is_reported_at_once() {
    let mut policy = policy();
    policy.tool_allow_list.insert(ToolId::CommandRun);
    policy.limits.wall_clock_seconds = 1;
    policy.limits.max_files_written = 0;
    assert_eq!(policy.validate().unwrap_err().len(), 3);
}

#[test]
fn a_stored_limit_wins_over_the_factory_value() {
    let mut policy = policy();
    policy.limits.wall_clock_seconds = 120;
    assert!(policy.validate().is_ok());
    assert_ne!(policy.limits, Limits::factory());
}

// ---- host names (BR-18.4) ----------------------------------------------------------------------

#[test]
fn br_18_4_host_names_are_normalised_and_checked() {
    for (input, expected) in [
        ("api.example.com", "api.example.com"),
        ("  API.Example.COM  ", "api.example.com"),
        ("*.example.com", "*.example.com"),
        ("ollama", "ollama"),
        ("a-b.example.co.uk", "a-b.example.co.uk"),
        ("xn--bcher-kva.example", "xn--bcher-kva.example"),
    ] {
        assert_eq!(
            HostName::parse(input).unwrap().as_str(),
            expected,
            "{input}"
        );
    }
    for bad in [
        "",
        "   ",
        "https://api.example.com",
        "api.example.com/path",
        "api.example.com:8080",
        "user@api.example.com",
        "api example.com",
        "api.example.com?x=1",
        "*.com",
        "*",
        "*.*.example.com",
        "a*.example.com",
        "-bad.example.com",
        "bad-.example.com",
        "under_score.example.com",
        "a..b.example.com",
        ".example.com",
        "example.com.",
        "bücher.example",
        "192.168.1.1",
        "10.0.0.1",
        "::1",
        "1.2.3",
    ] {
        assert!(HostName::parse(bad).is_err(), "{bad:?} must be refused");
    }
    assert!(HostName::parse(&format!("{}.example.com", "a".repeat(64))).is_err());
    assert!(HostName::parse(&"a.".repeat(130)).is_err());
}

#[test]
fn br_18_4_an_allow_list_is_unique_lower_case_limited_and_reports_every_bad_entry() {
    let rule =
        NetworkRule::allow_hosts(["B.example.com", "a.example.com", "b.EXAMPLE.com"]).unwrap();
    let NetworkRule::AllowHosts(set) = rule else {
        panic!("expected a list")
    };
    assert_eq!(
        set.iter().map(HostName::as_str).collect::<Vec<_>>(),
        vec!["a.example.com", "b.example.com"]
    );

    let issues =
        NetworkRule::allow_hosts(["ok.example.com", "https://bad", "1.2.3.4"]).unwrap_err();
    assert_eq!(issues.len(), 2);
    assert!(issues.iter().all(|issue| issue.field() == "network"));

    let many: Vec<String> = (0..=MAX_ALLOWED_HOSTS)
        .map(|n| format!("h{n}.example.com"))
        .collect();
    let issues = NetworkRule::allow_hosts(&many).unwrap_err();
    assert!(issues.contains(&PolicyIssue::TooManyHosts { max: 50 }));
    assert!(NetworkRule::allow_hosts(&many[..MAX_ALLOWED_HOSTS]).is_ok());
    assert_eq!(
        NetworkRule::allow_hosts(Vec::<String>::new()).unwrap(),
        NetworkRule::AllowHosts(Default::default())
    );
}

// ---- the ceiling (BR-18.8, A16) -----------------------------------------------------------------

fn session(mut base: SandboxPolicy) -> SandboxPolicy {
    base.scope = PolicyScope::Session;
    base
}

#[test]
fn br_18_8_a_session_policy_equal_to_the_instance_policy_is_within_it() {
    let instance = policy();
    assert!(session(instance.clone()).exceeds(&instance).is_empty());
}

#[test]
fn br_18_8_a_session_may_tighten_each_setting() {
    let instance = policy();
    let mut tighter = session(instance.clone());
    tighter.limits.wall_clock_seconds = 60;
    tighter.limits.max_files_written = 10;
    tighter.tool_allow_list.remove(&ToolId::WebSearch);
    tighter.runner_tier = RunnerTier::Container;
    assert!(tighter.exceeds(&instance).is_empty());
    assert!(tighter.validate().is_ok());
}

#[test]
fn br_18_8_a_session_cannot_exceed_the_instance_policy_on_any_setting() {
    let mut instance = policy();
    instance.limits.wall_clock_seconds = 100;
    instance.tool_allow_list.remove(&ToolId::RepoRead);
    instance.runner_tier = RunnerTier::Container;

    let mut over = session(instance.clone());
    over.limits.wall_clock_seconds = 200;
    over.tool_allow_list.insert(ToolId::RepoRead);
    over.runner_tier = RunnerTier::InProcess;
    over.network = hosts(&["api.example.com"]);
    let fields: Vec<&str> = over
        .exceeds(&instance)
        .iter()
        .map(PolicyIssue::field)
        .collect();
    assert_eq!(
        fields,
        vec![
            "wall_clock_seconds",
            "tool_allow_list",
            "runner_tier",
            "network"
        ]
    );
}

#[test]
fn br_18_8_network_rules_compare_by_coverage() {
    let mut instance = policy();
    instance.network = hosts(&["*.example.com", "api.other.test"]);
    let within = |entries: &[&str]| {
        let mut candidate = session(instance.clone());
        candidate.network = hosts(entries);
        candidate.exceeds(&instance).is_empty()
    };
    assert!(within(&["api.example.com"]));
    assert!(within(&["a.b.example.com"]));
    assert!(within(&["*.example.com"]));
    assert!(within(&["*.a.example.com"]));
    assert!(within(&["api.other.test"]));
    assert!(within(&[]), "an empty allow-list is within any list");
    assert!(
        !within(&["example.com"]),
        "a wildcard does not cover its own apex"
    );
    assert!(!within(&["evil.test"]));
    assert!(!within(&["*.other.test"]));
    assert!(!within(&["api.example.com.evil.test"]));
    assert!(
        !within(&["notexample.com"]),
        "suffix match must be on a label boundary"
    );
    // `none` is always within; a list is never within `none`.
    let mut none_session = session(instance.clone());
    none_session.network = NetworkRule::None;
    assert!(none_session.exceeds(&instance).is_empty());
    let mut listed = session(policy());
    listed.network = hosts(&["api.example.com"]);
    assert_eq!(listed.exceeds(&policy())[0].field(), "network");
}

#[test]
fn br_18_8_clamping_applies_a_lowered_instance_policy_to_an_existing_session_value() {
    let mut session_policy = session(policy());
    session_policy.limits.wall_clock_seconds = 900;
    session_policy.limits.max_output_bytes = 2 * 1_048_576;
    session_policy.network = hosts(&["a.example.com", "b.other.test"]);
    session_policy.version = 7;

    let mut lowered = policy();
    lowered.limits.wall_clock_seconds = 300;
    lowered.tool_allow_list.remove(&ToolId::WorkspaceWrite);
    lowered.runner_tier = RunnerTier::Container;
    lowered.network = hosts(&["*.example.com"]);

    let clamped = session_policy.clamped_to(&lowered);
    assert_eq!(clamped.scope, PolicyScope::Session);
    assert_eq!(
        clamped.limits.wall_clock_seconds, 300,
        "lowered to the new ceiling"
    );
    assert_eq!(
        clamped.limits.max_output_bytes, 1_048_576,
        "the ceiling's smaller value"
    );
    assert!(!clamped.tool_allow_list.contains(&ToolId::WorkspaceWrite));
    assert_eq!(clamped.runner_tier, RunnerTier::Container);
    assert_eq!(
        clamped.network,
        hosts(&["a.example.com"]),
        "b.other.test is no longer covered"
    );
    assert_eq!(clamped.version, 7);
    assert!(
        clamped.exceeds(&lowered).is_empty(),
        "a clamped policy is always within its ceiling"
    );
}

#[test]
fn br_18_8_clamping_to_no_network_removes_all_hosts_and_never_loosens() {
    let mut session_policy = session(policy());
    session_policy.network = hosts(&["a.example.com"]);
    let clamped = session_policy.clamped_to(&policy());
    assert_eq!(clamped.network, NetworkRule::None);

    let mut tight = session(policy());
    tight.limits.wall_clock_seconds = 40;
    tight.runner_tier = RunnerTier::Container;
    let again = tight.clamped_to(&policy());
    assert_eq!(
        again.limits.wall_clock_seconds, 40,
        "a tighter session value stays"
    );
    assert_eq!(
        again.runner_tier,
        RunnerTier::Container,
        "a stricter tier stays"
    );
}

#[test]
fn scopes_and_tiers_round_trip() {
    for tier in [RunnerTier::InProcess, RunnerTier::Container] {
        assert_eq!(RunnerTier::parse(tier.as_str()), Some(tier));
    }
    for scope in [PolicyScope::Instance, PolicyScope::Session] {
        assert_eq!(PolicyScope::parse(scope.as_str()), Some(scope));
    }
    assert_eq!(RunnerTier::parse("vm"), None);
    assert_eq!(PolicyScope::parse("global"), None);
}
