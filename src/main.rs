use ortyo::{
    cli::{Cli, Command, usage},
    domain::{Scenario, ScenarioOutcome, ScenarioRun},
    hosted_server::{HostedServerConfig, run_hosted_server},
    http::{AppState, app},
    scenario::{CreateScenario, ScenarioManifest, outcome_exit_code},
    scenario_run::{environment as scenario_environment, exit_code as scenario_run_exit_code, report as scenario_run_report},
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
        Command::ScenarioRun { path, command } => {
            scenario_run(&cli.base_url, &path, command).await
        }
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

fn load_scenario_manifest(path: &str) -> Result<CreateScenario, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("read Scenario manifest: {error}"))?;
    let manifest: ScenarioManifest = serde_json::from_str(&content)
        .map_err(|error| format!("parse Scenario manifest JSON: {error}"))?;
    Ok(manifest.into())
}

async fn scenario_create_request(base_url: &str, path: &str) -> Result<Scenario, String> {
    let request = load_scenario_manifest(path)?;
    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_value(response).await?;
    serde_json::from_value(value).map_err(|error| format!("invalid Scenario: {error}"))
}

async fn scenario_start_request(base_url: &str, id: uuid::Uuid) -> Result<ScenarioRun, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenarios/{id}/start"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_value(response).await?;
    serde_json::from_value(value).map_err(|error| format!("invalid ScenarioRun: {error}"))
}

async fn scenario_complete_request(
    base_url: &str,
    id: uuid::Uuid,
) -> Result<ScenarioOutcome, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenario-runs/{id}/complete"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_value(response).await?;
    serde_json::from_value(value).map_err(|error| format!("invalid ScenarioOutcome: {error}"))
}

async fn scenario_create(base_url: &str, path: &str) -> Result<(), String> {
    let scenario = scenario_create_request(base_url, path).await?;
    let value =
        serde_json::to_value(scenario).map_err(|error| format!("serialize Scenario: {error}"))?;
    print_json(&value)
}

async fn scenario_complete(base_url: &str, id: uuid::Uuid) -> Result<i32, String> {
    let outcome = scenario_complete_request(base_url, id).await?;
    let value = serde_json::to_value(&outcome)
        .map_err(|error| format!("serialize ScenarioOutcome: {error}"))?;
    print_json(&value)?;
    Ok(outcome_exit_code(&outcome))
}

async fn scenario_run(
    base_url: &str,
    path: &str,
    command: Vec<String>,
) -> Result<i32, String> {
    let scenario = scenario_create_request(base_url, path).await?;
    let run = scenario_start_request(base_url, scenario.id).await?;
    let program = command
        .first()
        .ok_or_else(|| "scenario run requires a child command".to_owned())?;

    let output = std::process::Command::new(program)
        .args(&command[1..])
        .envs(scenario_environment(base_url, &run))
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            let _ = scenario_complete_request(base_url, run.id).await;
            let _ = revoke_exposure(base_url, run.exposure_id).await;
            return Err(format!("run child command: {error}"));
        }
    };

    if !output.stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    let outcome = match scenario_complete_request(base_url, run.id).await {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = revoke_exposure(base_url, run.exposure_id).await;
            return Err(error);
        }
    };
    let report = scenario_run_report(&run, command, output.status.code(), outcome);
    let value = serde_json::to_value(&report)
        .map_err(|error| format!("serialize ScenarioRunReport: {error}"))?;
    print_json(&value)?;
    Ok(scenario_run_exit_code(&report))
}

async fn revoke_exposure(base_url: &str, id: uuid::Uuid) -> Result<(), String> {
    let response = reqwest::Client::new()
        .delete(format!("{base_url}/_ortyo/exposures/{id}"))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if response.status().is_success() || response.status() == reqwest::StatusCode::GONE {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;
        Err(format!("HTTP {status}: {body}"))
    }
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
