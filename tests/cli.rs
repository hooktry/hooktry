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
