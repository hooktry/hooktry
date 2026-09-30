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
