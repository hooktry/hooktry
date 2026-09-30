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
    pub runtime_state: &'static str,
}

#[derive(Clone, Default)]
pub struct HostedRuntimeManager {
    inner: Arc<Mutex<HashMap<Uuid, AbortHandle>>>,
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
        let runtime_state = state.clone();
        let task = tokio::spawn(async move {
            maintain_websocket_runtime(
                connection,
                runtime_url,
                exposure_id,
                capability,
                runtime_state,
                Duration::from_secs(1),
            )
            .await;
        });

        let previous = self
            .inner
            .lock()
            .expect("hosted runtime store poisoned")
            .insert(exposure_id, task.abort_handle());
        if let Some(previous) = previous {
            previous.abort();
        }

        Ok(HostedRuntimeStatus {
            exposure_id,
            name: exposure.name,
            url: exposure.url,
            access: exposure.access,
            mode: exposure.mode,
            target_port: exposure.target.port,
            verified: true,
            runtime_state: "connected",
        })
    }

    pub fn stop(&self, exposure_id: Uuid) {
        if let Some(handle) = self
            .inner
            .lock()
            .expect("hosted runtime store poisoned")
            .remove(&exposure_id)
        {
            handle.abort();
        }
    }
}
