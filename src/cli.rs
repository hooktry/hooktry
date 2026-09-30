use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Interactions,
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

pub fn usage() -> String {
    "usage: ortyo [--base-url URL] <serve|interactions|assert CONTRACT_ID INTERACTION_ID>".to_owned()
}
