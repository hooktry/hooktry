use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::{Duration, timeout},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayRequest {
    pub id: Uuid,
    pub exposure_id: Uuid,
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayResponse {
    pub request_id: Uuid,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayError {
    RuntimeUnavailable,
    RuntimeDisconnected,
    ResponseDropped,
    Timeout,
}

struct RelayDispatch {
    request: RelayRequest,
    response_tx: oneshot::Sender<RelayResponse>,
}

#[derive(Clone, Default)]
pub struct RelayBroker {
    runtimes: Arc<Mutex<HashMap<Uuid, mpsc::Sender<RelayDispatch>>>>,
}

impl RelayBroker {
    pub async fn register(&self, exposure_id: Uuid) -> RelayRuntime {
        let (tx, rx) = mpsc::channel(16);
        self.runtimes.lock().await.insert(exposure_id, tx);
        RelayRuntime {
            exposure_id,
            receiver: rx,
        }
    }

    pub async fn ingress(&self, request: RelayRequest) -> Result<RelayResponse, RelayError> {
        self.ingress_with_timeout(request, Duration::from_secs(30)).await
    }

    pub async fn ingress_with_timeout(
        &self,
        request: RelayRequest,
        deadline: Duration,
    ) -> Result<RelayResponse, RelayError> {
        let runtime = self
            .runtimes
            .lock()
            .await
            .get(&request.exposure_id)
            .cloned()
            .ok_or(RelayError::RuntimeUnavailable)?;

        let (response_tx, response_rx) = oneshot::channel();
        runtime
            .send(RelayDispatch {
                request,
                response_tx,
            })
            .await
            .map_err(|_| RelayError::RuntimeDisconnected)?;

        timeout(deadline, response_rx)
            .await
            .map_err(|_| RelayError::Timeout)?
            .map_err(|_| RelayError::ResponseDropped)
    }
}

pub struct RelayRuntime {
    exposure_id: Uuid,
    receiver: mpsc::Receiver<RelayDispatch>,
}

impl RelayRuntime {
    pub fn exposure_id(&self) -> Uuid {
        self.exposure_id
    }

    pub async fn recv(&mut self) -> Option<RelayWork> {
        self.receiver.recv().await.map(|dispatch| RelayWork {
            request: dispatch.request,
            response_tx: Some(dispatch.response_tx),
        })
    }
}

pub struct RelayWork {
    pub request: RelayRequest,
    response_tx: Option<oneshot::Sender<RelayResponse>>,
}

impl RelayWork {
    pub fn complete(mut self, response: RelayResponse) -> Result<(), RelayError> {
        self.response_tx
            .take()
            .expect("relay response sender missing")
            .send(response)
            .map_err(|_| RelayError::ResponseDropped)
    }
}
