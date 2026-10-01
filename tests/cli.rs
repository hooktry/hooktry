use ortyo::cli::{Cli, Command};
use uuid::Uuid;

#[test]
fn defaults_to_serve_for_backwards_compatibility() {
    let cli = Cli::parse(["ortyo".to_owned()]).unwrap();

    assert_eq!(cli.base_url, "http://127.0.0.1:7777");
    assert_eq!(cli.command, Command::Serve);
}

#[test]
fn parses_machine_readable_interactions_command() {
    let cli = Cli::parse([
        "ortyo".to_owned(),
        "--base-url".to_owned(),
        "http://ortyo.test".to_owned(),
        "interactions".to_owned(),
    ])
    .unwrap();

    assert_eq!(cli.base_url, "http://ortyo.test");
    assert_eq!(cli.command, Command::Interactions);
}

#[test]
fn parses_assertion_ids() {
    let contract_id = Uuid::now_v7();
    let interaction_id = Uuid::now_v7();
    let cli = Cli::parse([
        "ortyo".to_owned(),
        "assert".to_owned(),
        contract_id.to_string(),
        interaction_id.to_string(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::Assert {
            contract_id,
            interaction_id
        }
    );
}

#[test]
fn rejects_invalid_assertion_ids() {
    let error = Cli::parse([
        "ortyo".to_owned(),
        "assert".to_owned(),
        "not-a-uuid".to_owned(),
        Uuid::now_v7().to_string(),
    ])
    .unwrap_err();

    assert_eq!(error, "assert requires a valid contract UUID");
}

#[test]
fn parses_approval_and_execution_commands() {
    let approval_id = Uuid::now_v7();
    let execution_id = Uuid::now_v7();

    let inbox = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "inbox".to_owned(),
    ])
    .unwrap();
    assert_eq!(inbox.command, Command::ApprovalInbox);

    let create = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "create".to_owned(),
        "request.json".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        create.command,
        Command::ApprovalCreate {
            path: "request.json".to_owned()
        }
    );

    let get = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "get".to_owned(),
        approval_id.to_string(),
    ])
    .unwrap();
    assert_eq!(get.command, Command::ApprovalGet { id: approval_id });

    let approve = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "approve".to_owned(),
        approval_id.to_string(),
    ])
    .unwrap();
    assert_eq!(
        approve.command,
        Command::ApprovalApprove { id: approval_id }
    );

    let deny = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "deny".to_owned(),
        approval_id.to_string(),
    ])
    .unwrap();
    assert_eq!(deny.command, Command::ApprovalDeny { id: approval_id });

    let execute = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "execute".to_owned(),
        approval_id.to_string(),
        "request.json".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        execute.command,
        Command::ApprovalExecute {
            id: approval_id,
            path: "request.json".to_owned()
        }
    );

    let execution = Cli::parse([
        "ortyo".to_owned(),
        "execution".to_owned(),
        "get".to_owned(),
        execution_id.to_string(),
    ])
    .unwrap();
    assert_eq!(
        execution.command,
        Command::ExecutionGet { id: execution_id }
    );
}

#[test]
fn rejects_invalid_approval_and_execution_ids() {
    let approval = Cli::parse([
        "ortyo".to_owned(),
        "approval".to_owned(),
        "approve".to_owned(),
        "not-a-uuid".to_owned(),
    ])
    .unwrap_err();
    assert_eq!(approval, "approval approve requires a valid UUID");

    let execution = Cli::parse([
        "ortyo".to_owned(),
        "execution".to_owned(),
        "get".to_owned(),
        "not-a-uuid".to_owned(),
    ])
    .unwrap_err();
    assert_eq!(execution, "execution get requires a valid UUID");
}

#[test]
fn parses_mcp_stdio_command() {
    let cli = Cli::parse(["ortyo".to_owned(), "mcp".to_owned()]).unwrap();

    assert_eq!(cli.command, Command::Mcp);
}

#[test]
fn parses_scenario_manifest_lifecycle_commands() {
    let scenario_id = Uuid::now_v7();
    let run_id = Uuid::now_v7();

    let create = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "create".to_owned(),
        "ortyo/payment-webhook.json".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        create.command,
        Command::ScenarioCreate {
            path: "ortyo/payment-webhook.json".to_owned()
        }
    );

    let get = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "get".to_owned(),
        scenario_id.to_string(),
    ])
    .unwrap();
    assert_eq!(get.command, Command::ScenarioGet { id: scenario_id });

    let start = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "start".to_owned(),
        scenario_id.to_string(),
    ])
    .unwrap();
    assert_eq!(start.command, Command::ScenarioStart { id: scenario_id });

    let complete = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "complete".to_owned(),
        run_id.to_string(),
    ])
    .unwrap();
    assert_eq!(complete.command, Command::ScenarioComplete { id: run_id });

    let outcome = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "outcome".to_owned(),
        run_id.to_string(),
    ])
    .unwrap();
    assert_eq!(outcome.command, Command::ScenarioOutcome { id: run_id });
}

#[test]
fn rejects_invalid_scenario_ids() {
    let error = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "complete".to_owned(),
        "not-a-uuid".to_owned(),
    ])
    .unwrap_err();

    assert_eq!(error, "scenario complete requires a valid UUID");
}

#[test]
fn parses_scenario_run_with_explicit_child_command() {
    let cli = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "run".to_owned(),
        "ortyo/payment-webhook.json".to_owned(),
        "--".to_owned(),
        "bundle".to_owned(),
        "exec".to_owned(),
        "ruby".to_owned(),
        "test/webhook_test.rb".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::ScenarioRun {
            path: "ortyo/payment-webhook.json".to_owned(),
            command: vec![
                "bundle".to_owned(),
                "exec".to_owned(),
                "ruby".to_owned(),
                "test/webhook_test.rb".to_owned(),
            ],
        }
    );
}

#[test]
fn scenario_run_requires_a_child_command_after_separator() {
    let error = Cli::parse([
        "ortyo".to_owned(),
        "scenario".to_owned(),
        "run".to_owned(),
        "scenario.json".to_owned(),
        "--".to_owned(),
    ])
    .unwrap_err();

    assert_eq!(error, ortyo::cli::usage());
}


#[test]
fn parses_key_maintenance_commands() {
    let status = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "status".to_owned(),
    ])
    .unwrap();
    assert_eq!(status.command, Command::KeyStatus);

    let dry_run = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "rewrap".to_owned(),
        "1".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        dry_run.command,
        Command::KeyRewrap {
            version: 1,
            apply: false
        }
    );

    let apply = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "rewrap".to_owned(),
        "1".to_owned(),
        "--apply".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        apply.command,
        Command::KeyRewrap {
            version: 1,
            apply: true
        }
    );

    let retire = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "retire-check".to_owned(),
        "1".to_owned(),
    ])
    .unwrap();
    assert_eq!(
        retire.command,
        Command::KeyRetireCheck { version: 1 }
    );
}

#[test]
fn key_maintenance_requires_positive_versions() {
    let rewrap = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "rewrap".to_owned(),
        "0".to_owned(),
    ])
    .unwrap_err();
    assert_eq!(rewrap, "key rewrap requires a positive key version");

    let retire = Cli::parse([
        "ortyo".to_owned(),
        "key".to_owned(),
        "retire-check".to_owned(),
        "-1".to_owned(),
    ])
    .unwrap_err();
    assert_eq!(retire, "key retire-check requires a positive key version");
}
