use ortyo::{
    cli::{Cli, Command, usage},
    domain::ScenarioOutcome,
    hosted_server::{HostedServerConfig, run_hosted_server},
    http::{AppState, app},
    scenario::{CreateScenario, ScenarioManifest, outcome_exit_code},
    store::InteractionStore,
};

#[tokio::main]
async fn main() {
    let cli = match Cli::parse(std::env::args()) {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let result = match cli.command {
        Command::Serve => serve().await.map(|_| 0),
        Command::Hosted => hosted().await.map(|_| 0),
        Command::Interactions => get_json(&format!("{}/_ortyo/interactions", cli.base_url))
            .await
            .map(|_| 0),
        Command::Exposures => get_json(&format!("{}/_ortyo/exposures", cli.base_url))
            .await
            .map(|_| 0),
        Command::ExposureGet { id } => get_json(&format!("{}/_ortyo/exposures/{id}", cli.base_url))
            .await
            .map(|_| 0),
        Command::ExposureRevoke { id } => {
            delete_json(&format!("{}/_ortyo/exposures/{id}", cli.base_url))
                .await
                .map(|_| 0)
        }
        Command::Expose { name, port, verify } => {
            expose(&cli.base_url, &name, port, verify).await.map(|_| 0)
        }
        Command::Mcp => ortyo::mcp::run_stdio(&cli.base_url).await.map(|_| 0),
        Command::ScenarioCreate { path } => scenario_create(&cli.base_url, &path).await.map(|_| 0),
        Command::ScenarioGet { id } => get_json(&format!("{}/_ortyo/scenarios/{id}", cli.base_url))
            .await
            .map(|_| 0),
        Command::ScenarioStart { id } => {
            post_json(&format!("{}/_ortyo/scenarios/{id}/start", cli.base_url))
                .await
                .map(|_| 0)
        }
        Command::ScenarioComplete { id } => scenario_complete(&cli.base_url, id).await,
        Command::ScenarioOutcome { id } => get_json(&format!(
            "{}/_ortyo/scenario-runs/{id}/outcome",
            cli.base_url
        ))
        .await
        .map(|_| 0),
        Command::Assert {
            contract_id,
            interaction_id,
        } => post_assertion(&cli.base_url, contract_id, interaction_id).await,
    };

    match result {
        Ok(code) if code != 0 => std::process::exit(code),
        Ok(_) => {}
        Err(error) => {
            eprintln!("ortyo: {error}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

async fn hosted() -> Result<(), String> {
    let config = HostedServerConfig::from_lookup(|key| std::env::var(key).ok())?;
    run_hosted_server(config).await
}

async fn serve() -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7777")
        .await
        .map_err(|error| format!("bind ORTYO HTTP boundary: {error}"))?;
    let state = AppState {
        store: InteractionStore::open("ortyo.db")
            .map_err(|error| format!("open ORTYO evidence database: {error}"))?,
        ..AppState::default()
    };

    println!("ORTYO HTTP boundary: http://127.0.0.1:7777");
    axum::serve(listener, app(state))
        .await
        .map_err(|error| format!("serve ORTYO: {error}"))
}

async fn get_json(url: &str) -> Result<(), String> {
    emit_response(reqwest::get(url).await.map_err(|error| error.to_string())?).await
}

async fn expose(base_url: &str, name: &str, port: u16, verify: bool) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/exposures"))
        .json(&serde_json::json!({
            "name": name,
            "port": port,
            "mode": "forward",
            "access": "private"
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let exposure: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("invalid exposure response: {error}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {exposure}"));
    }

    let url = exposure["url"]
        .as_str()
        .ok_or_else(|| "exposure response is missing url".to_owned())?;
    let verified = if verify {
        reqwest::get(format!("{url}/_ortyo_verify"))
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "exposure_id": exposure["id"],
            "name": exposure["name"],
            "url": url,
            "access": exposure["access"],
            "mode": exposure["mode"],
            "target_port": port,
            "verified": verified
        }))
        .map_err(|error| error.to_string())?
    );
    Ok(())
}

async fn scenario_create(base_url: &str, path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("read Scenario manifest: {error}"))?;
    let manifest: ScenarioManifest = serde_json::from_str(&content)
        .map_err(|error| format!("parse Scenario manifest JSON: {error}"))?;
    let request: CreateScenario = manifest.into();

    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    emit_response(response).await
}

async fn scenario_complete(base_url: &str, id: uuid::Uuid) -> Result<i32, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenario-runs/{id}/complete"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_value(response).await?;
    let outcome: ScenarioOutcome = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid ScenarioOutcome: {error}"))?;
    print_json(&value)?;
    Ok(outcome_exit_code(&outcome))
}

async fn post_assertion(
    base_url: &str,
    contract_id: uuid::Uuid,
    interaction_id: uuid::Uuid,
) -> Result<i32, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{base_url}/_ortyo/contracts/{contract_id}/assert/{interaction_id}"
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_value(response).await?;
    let passed = value["passed"]
        .as_bool()
        .ok_or_else(|| "assertion response is missing passed".to_owned())?;
    print_json(&value)?;
    Ok(if passed { 0 } else { 1 })
}

async fn delete_json(url: &str) -> Result<(), String> {
    let response = reqwest::Client::new()
        .delete(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    emit_response(response).await
}

async fn post_json(url: &str) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    emit_response(response).await
}

async fn emit_response(response: reqwest::Response) -> Result<(), String> {
    let value = response_value(response).await?;
    print_json(&value)
}

async fn response_value(response: reqwest::Response) -> Result<serde_json::Value, String> {
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {body}"));
    }

    serde_json::from_str(&body).map_err(|error| format!("invalid JSON response: {error}"))
}

fn print_json(value: &serde_json::Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}
