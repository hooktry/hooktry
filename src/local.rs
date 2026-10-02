use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{Json, Router, routing::get};
use serde_json::json;

use crate::{
    anonymous::{AnonymousExposureService, AnonymousExposureStore, anonymous_app},
    hosted_identity::HostedIdentityStore,
    http::{AppState, app as api_app},
    store::InteractionStore,
    web_assets,
};

pub struct LocalServerConfig {
    pub bind: String,
    pub db_path: PathBuf,
    pub open_browser: bool,
}

impl Default for LocalServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:7777".to_owned(),
            db_path: PathBuf::from("hooktry.db"),
            open_browser: false,
        }
    }
}

struct LocalRuntime {
    router: Router,
    anonymous: AnonymousExposureService,
}

pub async fn run_local_server(config: LocalServerConfig) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("bind HOOKTRY local server: {error}"))?;
    let socket = listener
        .local_addr()
        .map_err(|error| format!("read HOOKTRY local socket: {error}"))?;
    let public_base_url = local_public_base_url(socket);
    let runtime = local_runtime(&config.db_path, &public_base_url)?;

    let cleanup = runtime.anonymous.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            if let Err(error) = cleanup.purge_expired().await {
                eprintln!("hooktry: anonymous expiry cleanup failed: {error:?}");
            }
        }
    });

    println!("HOOKTRY local UI: {public_base_url}");
    println!("HOOKTRY local API: {public_base_url}/api/v1/hooks");
    println!("HOOKTRY local data: {}", config.db_path.display());

    if config.open_browser {
        let url = public_base_url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            if let Err(error) = open_browser(&url) {
                eprintln!("hooktry: could not open browser: {error}");
                eprintln!("hooktry: open {url}");
            }
        });
    }

    axum::serve(listener, runtime.router)
        .await
        .map_err(|error| format!("serve HOOKTRY local server: {error}"))
}

pub fn local_app(db_path: &Path, public_base_url: &str) -> Result<Router, String> {
    Ok(local_runtime(db_path, public_base_url)?.router)
}

fn local_runtime(db_path: &Path, public_base_url: &str) -> Result<LocalRuntime, String> {
    let state = AppState {
        store: InteractionStore::open(db_path)
            .map_err(|error| format!("open HOOKTRY evidence database: {error}"))?,
        ..AppState::default()
    };
    let anonymous_store = AnonymousExposureStore::open(db_path)
        .map_err(|error| format!("open HOOKTRY anonymous Hook database: {error:?}"))?;
    let anonymous = AnonymousExposureService::new(anonymous_store, public_base_url);
    let router = api_app(state)
        .merge(anonymous_app(
            anonymous.clone(),
            HostedIdentityStore::default(),
        ))
        .route("/healthz", get(health))
        .fallback(web_assets::fallback);

    Ok(LocalRuntime { router, anonymous })
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "service": "hooktry-local"
    }))
}

fn local_public_base_url(socket: SocketAddr) -> String {
    let host = match socket.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_owned(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".to_owned(),
        IpAddr::V6(ip) => format!("[{ip}]"),
        IpAddr::V4(ip) => ip.to_string(),
    };
    format!("http://{host}:{}", socket.port())
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("run open: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|error| format!("run start: {error}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("run xdg-open: {error}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("browser opening is unsupported on this platform".to_owned())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::local_app;
    use crate::anonymous::AnonymousProvision;

    #[tokio::test]
    async fn local_app_serves_embedded_web_and_canonical_hook_flow() {
        let path = std::env::temp_dir().join(format!("hooktry-local-{}.db", Uuid::now_v7()));
        let app = local_app(&path, "http://127.0.0.1:7777").unwrap();

        let root = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root.status(), StatusCode::OK);
        let root_body = root.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&root_body).contains(r#"<div id="root"></div>"#));

        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/hooks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let create_body = create.into_body().collect().await.unwrap().to_bytes();
        let provision: AnonymousProvision = serde_json::from_slice(&create_body).unwrap();

        let view_path = url_path(&provision.view_url);
        let viewer = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(view_path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(viewer.status(), StatusCode::OK);
        let viewer_body = viewer.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&viewer_body).contains(r#"<div id="root"></div>"#));

        let hook_path = format!("{}/local1?source=test", url_path(&provision.hook_url));
        let captured = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(hook_path)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"hello":"local"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(captured.status(), StatusCode::OK);

        drop(app);
        let _ = std::fs::remove_file(path);
    }

    fn url_path(url: &str) -> String {
        let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
        let path = without_scheme
            .find('/')
            .map(|index| &without_scheme[index..])
            .unwrap_or("/");
        path.to_owned()
    }
}
