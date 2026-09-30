use ortyo::cli::{Cli, Command};

#[test]
fn expose_defaults_to_web_and_verification() {
    let cli = Cli::parse([
        "ortyo".to_owned(),
        "expose".to_owned(),
        "3000".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::Expose {
            name: "web".to_owned(),
            port: 3000,
            verify: true,
        }
    );
}

#[test]
fn expose_accepts_name_and_no_verify() {
    let cli = Cli::parse([
        "ortyo".to_owned(),
        "expose".to_owned(),
        "8080".to_owned(),
        "stripe".to_owned(),
        "--no-verify".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::Expose {
            name: "stripe".to_owned(),
            port: 8080,
            verify: false,
        }
    );
}

#[test]
fn exposure_management_commands_are_typed() {
    let id = uuid::Uuid::now_v7();

    assert_eq!(
        Cli::parse(["ortyo".to_owned(), "exposures".to_owned()])
            .unwrap()
            .command,
        Command::Exposures
    );
    assert_eq!(
        Cli::parse([
            "ortyo".to_owned(),
            "exposure-get".to_owned(),
            id.to_string(),
        ])
        .unwrap()
        .command,
        Command::ExposureGet { id }
    );
    assert_eq!(
        Cli::parse([
            "ortyo".to_owned(),
            "exposure-revoke".to_owned(),
            id.to_string(),
        ])
        .unwrap()
        .command,
        Command::ExposureRevoke { id }
    );
}
