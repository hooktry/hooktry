use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::task::AbortHandle;
use uuid::Uuid;

use crate::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget},
    exposure::{CreateExposure, ExposureError},
    hosted::ProvisionedExposure,
    http::AppState,
    websocket_transport::{connect_websocket_runtime, maintain_websocket_runtime},
};

#[derive(Debug)]
pub enum HostedRuntimeError {
    InvalidProvision,
    Exposure(ExposureError),
    Transport(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostedRuntimeStatus {
    pub exposure_id: Uuid,
    pub name: String,
    pub url: String,
    pub access: ExposureAccess,
    pub mode: ExposureMode,
    pub target_port: u16,
    pub verified: bool,
    pub runtime_state: String,
}

struct HostedRuntimeHandle {
    abort_handle: AbortHandle,
    revoke_url: String,
    capability: String,
}

#[derive(Clone, Default)]
pub struct HostedRuntimeManager {
    inner: Arc<Mutex<HashMap<Uuid, HostedRuntimeHandle>>>,
}

impl HostedRuntimeManager {
    pub async fn attach(
        &self,
        state: AppState,
        provision: ProvisionedExposure,
    ) -> Result<HostedRuntimeStatus, HostedRuntimeError> {
        if provision.mode != ExposureMode::Relay
            || provision.access != ExposureAccess::Public
            || !provision.public_url.starts_with("http")
            || !(provision.runtime_url.starts_with("ws://")
                || provision.runtime_url.starts_with("wss://"))
        {
            return Err(HostedRuntimeError::InvalidProvision);
        }

        let exposure = state
            .exposures
            .adopt_with_url(
                state.session.id,
                provision.exposure_id,
                CreateExposure {
                    name: provision.name.clone(),
                    target: ExposureTarget {
                        host: "127.0.0.1".to_owned(),
                        port: provision.target_port,
                    },
                    mode: ExposureMode::Relay,
                    access: ExposureAccess::Public,
                },
                provision.public_url.clone(),
            )
            .map_err(HostedRuntimeError::Exposure)?;

        let connection = match connect_websocket_runtime(
            &provision.runtime_url,
            provision.exposure_id,
            &provision.runtime_capability,
            state.clone(),
        )
        .await
        {
            Ok(connection) => connection,
            Err(error) => {
                let _ = state.exposures.revoke(provision.exposure_id);
                return Err(HostedRuntimeError::Transport(format!("{error:?}")));
            }
        };

        let exposure_id = provision.exposure_id;
        let runtime_url = provision.runtime_url;
        let capability = provision.runtime_capability;
        let revoke_url =
            runtime_http_url(&runtime_url).ok_or(HostedRuntimeError::InvalidProvision)?;
        let runtime_url_for_task = runtime_url.clone();
        let capability_for_task = capability.clone();
        let runtime_state = state.clone();
        let task = tokio::spawn(async move {
            maintain_websocket_runtime(
                connection,
                runtime_url_for_task,
                exposure_id,
                capability_for_task,
                runtime_state,
                Duration::from_secs(1),
            )
            .await;
        });

        let previous = self
            .inner
            .lock()
            .expect("hosted runtime store poisoned")
            .insert(
                exposure_id,
                HostedRuntimeHandle {
                    abort_handle: task.abort_handle(),
                    revoke_url,
                    capability,
                },
            );
        if let Some(previous) = previous {
            previous.abort_handle.abort();
        }

        Ok(HostedRuntimeStatus {
            exposure_id,
            name: exposure.name,
            url: exposure.url,
            access: exposure.access,
            mode: exposure.mode,
            target_port: exposure.target.port,
            verified: true,
            runtime_state: "connected".to_owned(),
        })
    }

    pub async fn stop(&self, exposure_id: Uuid) {
        let runtime = self
            .inner
            .lock()
            .expect("hosted runtime store poisoned")
            .remove(&exposure_id);

        if let Some(runtime) = runtime {
            let revoke = reqwest::Client::new()
                .delete(&runtime.revoke_url)
                .bearer_auth(&runtime.capability)
                .send();
            let _ = tokio::time::timeout(Duration::from_secs(3), revoke).await;
            runtime.abort_handle.abort();
        }
    }
}

fn runtime_http_url(runtime_url: &str) -> Option<String> {
    runtime_url
        .strip_prefix("wss://")
        .map(|rest| format!("https://{rest}"))
        .or_else(|| {
            runtime_url
                .strip_prefix("ws://")
                .map(|rest| format!("http://{rest}"))
        })
}
