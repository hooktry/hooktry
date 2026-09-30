use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Interactions,
    Expose {
        name: String,
        port: u16,
        verify: bool,
    },
    Exposures,
    ExposureGet { id: Uuid },
    ExposureRevoke { id: Uuid },
    Mcp,
    Assert {
        contract_id: Uuid,
        interaction_id: Uuid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub base_url: String,
    pub command: Command,
}

impl Cli {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let _program = args.next();
        let mut base_url = "http://127.0.0.1:7777".to_owned();
        let mut remaining = Vec::new();

        while let Some(arg) = args.next() {
            if arg == "--base-url" {
                base_url = args
                    .next()
                    .ok_or_else(|| "--base-url requires a value".to_owned())?;
            } else {
                remaining.push(arg);
                remaining.extend(args);
                break;
            }
        }

        let command = match remaining.as_slice() {
            [command] if command == "serve" => Command::Serve,
            [command] if command == "interactions" => Command::Interactions,
            [command] if command == "exposures" => Command::Exposures,
            [command, id] if command == "exposure-get" => Command::ExposureGet {
                id: id
                    .parse()
                    .map_err(|_| "exposure-get requires a valid UUID".to_owned())?,
            },
            [command, id] if command == "exposure-revoke" => Command::ExposureRevoke {
                id: id
                    .parse()
                    .map_err(|_| "exposure-revoke requires a valid UUID".to_owned())?,
            },
            [command, port] if command == "expose" => Command::Expose {
                name: "web".to_owned(),
                port: parse_port(port)?,
                verify: true,
            },
            [command, port, name] if command == "expose" => Command::Expose {
                name: name.clone(),
                port: parse_port(port)?,
                verify: true,
            },
            [command, port, name, flag] if command == "expose" && flag == "--no-verify" => {
                Command::Expose {
                    name: name.clone(),
                    port: parse_port(port)?,
                    verify: false,
                }
            }
            [command] if command == "mcp" => Command::Mcp,
            [command, contract_id, interaction_id] if command == "assert" => Command::Assert {
                contract_id: contract_id
                    .parse()
                    .map_err(|_| "assert requires a valid contract UUID".to_owned())?,
                interaction_id: interaction_id
                    .parse()
                    .map_err(|_| "assert requires a valid interaction UUID".to_owned())?,
            },
            [] => Command::Serve,
            _ => return Err(usage()),
        };

        Ok(Self { base_url, command })
    }
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "expose requires a valid non-zero port".to_owned())
}

pub fn usage() -> String {
    "usage: ortyo [--base-url URL] <serve|mcp|interactions|expose PORT [NAME] [--no-verify]|exposures|exposure-get ID|exposure-revoke ID|assert CONTRACT_ID INTERACTION_ID>"
        .to_owned()
}
