use ortyo::{
    cli::{Cli, Command, usage},
    http::{AppState, app},
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
        Command::Serve => serve().await,
        Command::Interactions => get_json(&format!("{}/_ortyo/interactions", cli.base_url)).await,
        Command::Exposures => get_json(&format!("{}/_ortyo/exposures", cli.base_url)).await,
        Command::ExposureGet { id } => {
            get_json(&format!("{}/_ortyo/exposures/{id}", cli.base_url)).await
        }
        Command::ExposureRevoke { id } => {
            delete_json(&format!("{}/_ortyo/exposures/{id}", cli.base_url)).await
        }
        Command::Expose { name, port, verify } => {
            expose(&cli.base_url, &name, port, verify).await
        }
        Command::Mcp => ortyo::mcp::run_stdio(&cli.base_url).await,
        Command::Assert {
            contract_id,
            interaction_id,
        } => {
            post_json(&format!(
                "{}/_ortyo/contracts/{contract_id}/assert/{interaction_id}",
                cli.base_url
            ))
            .await
        }
    };

    if let Err(error) = result {
        eprintln!("ortyo: {error}");
        eprintln!("{}", usage());
        std::process::exit(1);
    }
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
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {body}"));
    }

    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("invalid JSON response: {error}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
    );
    Ok(())
}
