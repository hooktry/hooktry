use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Ui,
    Hosted,
    Interactions,
    Expose {
        name: String,
        port: u16,
        verify: bool,
        public: bool,
    },
    Exposures,
    ExposureGet {
        id: Uuid,
    },
    ExposureRevoke {
        id: Uuid,
    },
    ApprovalInbox,
    ApprovalCreate {
        path: String,
    },
    ApprovalGet {
        id: Uuid,
    },
    ApprovalApprove {
        id: Uuid,
    },
    ApprovalDeny {
        id: Uuid,
    },
    ApprovalExecute {
        id: Uuid,
        path: String,
    },
    ExecutionGet {
        id: Uuid,
    },
    KeyStatus,
    KeyRewrap {
        version: i32,
        apply: bool,
    },
    KeyRetireCheck {
        version: i32,
    },
    Mcp,
    ScenarioCreate {
        path: String,
    },
    ScenarioRun {
        path: String,
        command: Vec<String>,
    },
    ScenarioGet {
        id: Uuid,
    },
    ScenarioStart {
        id: Uuid,
    },
    ScenarioComplete {
        id: Uuid,
    },
    ScenarioOutcome {
        id: Uuid,
    },
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

        let command = if remaining.first().is_some_and(|command| command == "expose") {
            parse_expose(&remaining[1..])?
        } else {
            match remaining.as_slice() {
                [command] if command == "serve" => Command::Serve,
                [command] if command == "ui" => Command::Ui,
                [command] if command == "hosted" => Command::Hosted,
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
                [group, command] if group == "approval" && command == "inbox" => {
                    Command::ApprovalInbox
                }
                [group, command, path] if group == "approval" && command == "create" => {
                    Command::ApprovalCreate { path: path.clone() }
                }
                [group, command, id] if group == "approval" && command == "get" => {
                    Command::ApprovalGet {
                        id: parse_uuid(id, "approval get")?,
                    }
                }
                [group, command, id] if group == "approval" && command == "approve" => {
                    Command::ApprovalApprove {
                        id: parse_uuid(id, "approval approve")?,
                    }
                }
                [group, command, id] if group == "approval" && command == "deny" => {
                    Command::ApprovalDeny {
                        id: parse_uuid(id, "approval deny")?,
                    }
                }
                [group, command, id, path] if group == "approval" && command == "execute" => {
                    Command::ApprovalExecute {
                        id: parse_uuid(id, "approval execute")?,
                        path: path.clone(),
                    }
                }
                [group, command, id] if group == "execution" && command == "get" => {
                    Command::ExecutionGet {
                        id: parse_uuid(id, "execution get")?,
                    }
                }
                [group, command] if group == "key" && command == "status" => Command::KeyStatus,
                [group, command, version] if group == "key" && command == "rewrap" => {
                    Command::KeyRewrap {
                        version: parse_key_version(version, "key rewrap")?,
                        apply: false,
                    }
                }
                [group, command, version, flag]
                    if group == "key" && command == "rewrap" && flag == "--apply" =>
                {
                    Command::KeyRewrap {
                        version: parse_key_version(version, "key rewrap")?,
                        apply: true,
                    }
                }
                [group, command, version] if group == "key" && command == "retire-check" => {
                    Command::KeyRetireCheck {
                        version: parse_key_version(version, "key retire-check")?,
                    }
                }
                [command] if command == "mcp" => Command::Mcp,
                [group, command, path] if group == "scenario" && command == "create" => {
                    Command::ScenarioCreate { path: path.clone() }
                }
                [group, command, path, separator, child @ ..]
                    if group == "scenario"
                        && command == "run"
                        && separator == "--"
                        && !child.is_empty() =>
                {
                    Command::ScenarioRun {
                        path: path.clone(),
                        command: child.to_vec(),
                    }
                }
                [group, command, id] if group == "scenario" && command == "get" => {
                    Command::ScenarioGet {
                        id: parse_uuid(id, "scenario get")?,
                    }
                }
                [group, command, id] if group == "scenario" && command == "start" => {
                    Command::ScenarioStart {
                        id: parse_uuid(id, "scenario start")?,
                    }
                }
                [group, command, id] if group == "scenario" && command == "complete" => {
                    Command::ScenarioComplete {
                        id: parse_uuid(id, "scenario complete")?,
                    }
                }
                [group, command, id] if group == "scenario" && command == "outcome" => {
                    Command::ScenarioOutcome {
                        id: parse_uuid(id, "scenario outcome")?,
                    }
                }
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
            }
        };

        Ok(Self { base_url, command })
    }
}

fn parse_expose(args: &[String]) -> Result<Command, String> {
    let port = args
        .first()
        .ok_or_else(usage)
        .and_then(|value| parse_port(value))?;

    let mut name = "web".to_owned();
    let mut name_seen = false;
    let mut verify = true;
    let mut public = false;

    for arg in &args[1..] {
        match arg.as_str() {
            "--no-verify" => verify = false,
            "--public" => public = true,
            flag if flag.starts_with('-') => return Err(format!("unknown expose flag: {flag}")),
            value if !name_seen => {
                name = value.to_owned();
                name_seen = true;
            }
            _ => return Err("expose accepts at most one name".to_owned()),
        }
    }

    Ok(Command::Expose {
        name,
        port,
        verify,
        public,
    })
}

fn parse_uuid(value: &str, command: &str) -> Result<Uuid, String> {
    value
        .parse()
        .map_err(|_| format!("{command} requires a valid UUID"))
}

fn parse_key_version(value: &str, command: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| format!("{command} requires a positive key version"))
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "expose requires a valid non-zero port".to_owned())
}

pub fn usage() -> String {
    "usage: hooktry [--base-url URL] <serve|ui|hosted|mcp|interactions|expose PORT [NAME] [--public] [--no-verify]|exposures|exposure-get ID|exposure-revoke ID|approval inbox|approval create FILE|approval get ID|approval approve ID|approval deny ID|approval execute ID FILE|execution get ID|key status|key rewrap VERSION [--apply]|key retire-check VERSION|scenario create FILE|scenario run FILE -- COMMAND [ARGS...]|scenario get ID|scenario start ID|scenario complete RUN_ID|scenario outcome RUN_ID|assert CONTRACT_ID INTERACTION_ID>"
        .to_owned()
}
