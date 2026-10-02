use hooktry::cli::{Cli, Command};

#[test]
fn expose_defaults_to_web_and_verification() {
    let cli = Cli::parse(["hooktry".to_owned(), "expose".to_owned(), "3000".to_owned()]).unwrap();

    assert_eq!(
        cli.command,
        Command::Expose {
            name: "web".to_owned(),
            port: 3000,
            verify: true,
            public: false,
        }
    );
}

#[test]
fn expose_accepts_name_and_no_verify() {
    let cli = Cli::parse([
        "hooktry".to_owned(),
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
            public: false,
        }
    );
}

#[test]
fn exposure_management_commands_are_typed() {
    let id = uuid::Uuid::now_v7();

    assert_eq!(
        Cli::parse(["hooktry".to_owned(), "exposures".to_owned()])
            .unwrap()
            .command,
        Command::Exposures
    );
    assert_eq!(
        Cli::parse([
            "hooktry".to_owned(),
            "exposure-get".to_owned(),
            id.to_string(),
        ])
        .unwrap()
        .command,
        Command::ExposureGet { id }
    );
    assert_eq!(
        Cli::parse([
            "hooktry".to_owned(),
            "exposure-revoke".to_owned(),
            id.to_string(),
        ])
        .unwrap()
        .command,
        Command::ExposureRevoke { id }
    );
}

#[test]
fn hosted_command_is_typed() {
    let cli = Cli::parse(["hooktry".to_owned(), "hosted".to_owned()]).unwrap();
    assert_eq!(cli.command, Command::Hosted);
}

#[test]
fn expose_public_is_typed_and_flags_can_follow_the_name() {
    let cli = Cli::parse([
        "hooktry".to_owned(),
        "expose".to_owned(),
        "3000".to_owned(),
        "stripe".to_owned(),
        "--public".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::Expose {
            name: "stripe".to_owned(),
            port: 3000,
            verify: true,
            public: true,
        }
    );
}

#[test]
fn expose_public_can_use_default_name() {
    let cli = Cli::parse([
        "hooktry".to_owned(),
        "expose".to_owned(),
        "3000".to_owned(),
        "--public".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        Command::Expose {
            name: "web".to_owned(),
            port: 3000,
            verify: true,
            public: true,
        }
    );
}
