use std::{io, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::sleep,
};
use uuid::Uuid;

use crate::{
    http::{AppState, proxy_relay_request},
    relay::{RelayBroker, RelayRequest, RelayResponse},
    relay_auth::CapabilityStore,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayFrame {
    Register {
        exposure_id: Uuid,
        capability: String,
    },
    Registered {
        exposure_id: Uuid,
    },
    Request {
        request: RelayRequest,
    },
    Response {
        response: RelayResponse,
    },
    Ping {
        nonce: Uuid,
    },
    Pong {
        nonce: Uuid,
    },
    Error {
        message: String,
    },
}

#[derive(Debug)]
pub enum TransportError {
    Io(io::Error),
    Json(serde_json::Error),
    Protocol(String),
}

impl From<io::Error> for TransportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for TransportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub async fn serve_connection(
    stream: TcpStream,
    broker: RelayBroker,
    capabilities: CapabilityStore,
) -> Result<(), TransportError> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let writer = Arc::new(Mutex::new(write_half));

    let exposure_id = match read_frame::<RelayFrame, _>(&mut reader).await? {
        Some(RelayFrame::Register {
            exposure_id,
            capability,
        }) => {
            capabilities
                .authorize(exposure_id, &capability)
                .map_err(|error| {
                    TransportError::Protocol(format!("registration denied: {error:?}"))
                })?;
            exposure_id
        }
        Some(_) => {
            return Err(TransportError::Protocol(
                "expected register frame".to_owned(),
            ));
        }
        None => return Ok(()),
    };

    let mut runtime = broker.register(exposure_id).await;
    let registration_id = runtime.registration_id();
    write_frame(
        &mut *writer.lock().await,
        &RelayFrame::Registered { exposure_id },
    )
    .await?;

    let pending = Arc::new(Mutex::new(std::collections::HashMap::<
        Uuid,
        tokio::sync::oneshot::Sender<RelayResponse>,
    >::new()));
    let read_pending = pending.clone();
    let read_writer = writer.clone();

    let reader_task = tokio::spawn(async move {
        while let Some(frame) = read_frame::<RelayFrame, _>(&mut reader).await? {
            match frame {
                RelayFrame::Response { response } => {
                    if let Some(tx) = read_pending.lock().await.remove(&response.request_id) {
                        let _ = tx.send(response);
                    }
                }
                RelayFrame::Ping { nonce } => {
                    write_frame(&mut *read_writer.lock().await, &RelayFrame::Pong { nonce })
                        .await?;
                }
                _ => {}
            }
        }
        Ok::<(), TransportError>(())
    });

    while let Some(work) = runtime.recv().await {
        let request_id = work.request.id;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        pending.lock().await.insert(request_id, response_tx);
        write_frame(
            &mut *writer.lock().await,
            &RelayFrame::Request {
                request: work.request.clone(),
            },
        )
        .await?;

        let pending = pending.clone();
        tokio::spawn(async move {
            match response_rx.await {
                Ok(response) => {
                    let _ = work.complete(response);
                }
                Err(_) => {
                    pending.lock().await.remove(&request_id);
                }
            }
        });
    }

    reader_task.abort();
    broker.unregister(exposure_id, registration_id).await;
    Ok(())
}

pub async fn run_runtime_connection(
    stream: TcpStream,
    exposure_id: Uuid,
    capability: &str,
    state: AppState,
) -> Result<(), TransportError> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let writer = Arc::new(Mutex::new(write_half));

    write_frame(
        &mut *writer.lock().await,
        &RelayFrame::Register {
            exposure_id,
            capability: capability.to_owned(),
        },
    )
    .await?;
    match read_frame::<RelayFrame, _>(&mut reader).await? {
        Some(RelayFrame::Registered {
            exposure_id: registered,
        }) if registered == exposure_id => {}
        Some(frame) => {
            return Err(TransportError::Protocol(format!(
                "unexpected registration response: {frame:?}"
            )));
        }
        None => return Err(TransportError::Protocol("relay disconnected".to_owned())),
    }

    while let Some(frame) = read_frame::<RelayFrame, _>(&mut reader).await? {
        match frame {
            RelayFrame::Request { request } => {
                let state = state.clone();
                let writer = writer.clone();
                tokio::spawn(async move {
                    if let Ok(response) = proxy_relay_request(&state, &request).await {
                        let _ = write_frame(
                            &mut *writer.lock().await,
                            &RelayFrame::Response { response },
                        )
                        .await;
                    }
                });
            }
            RelayFrame::Ping { nonce } => {
                write_frame(&mut *writer.lock().await, &RelayFrame::Pong { nonce }).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn run_runtime_reconnecting(
    relay_addr: &str,
    exposure_id: Uuid,
    capability: &str,
    state: AppState,
    retry_delay: Duration,
) -> Result<(), TransportError> {
    let mut delay = retry_delay;
    loop {
        match TcpStream::connect(relay_addr).await {
            Ok(stream) => {
                let _ =
                    run_runtime_connection(stream, exposure_id, capability, state.clone()).await;
                delay = retry_delay;
            }
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                return Err(TransportError::Io(error));
            }
            Err(_) => {}
        }
        sleep(delay).await;
        delay = std::cmp::min(delay.saturating_mul(2), Duration::from_secs(30));
    }
}

pub async fn serve_listener(
    listener: TcpListener,
    broker: RelayBroker,
    capabilities: CapabilityStore,
) -> Result<(), TransportError> {
    loop {
        let (stream, _) = listener.accept().await?;
        let broker = broker.clone();
        let capabilities = capabilities.clone();
        tokio::spawn(async move {
            let _ = serve_connection(stream, broker, capabilities).await;
        });
    }
}

pub async fn heartbeat<W>(writer: &mut W, nonce: Uuid) -> Result<(), TransportError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    write_frame(writer, &RelayFrame::Ping { nonce }).await
}

async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), TransportError>
where
    W: tokio::io::AsyncWrite + Unpin,
    T: Serialize,
{
    let mut payload = serde_json::to_vec(value)?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_frame<T, R>(reader: &mut R) -> Result<Option<T>, TransportError>
where
    T: DeserializeOwned,
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut payload = Vec::new();
    let read = reader.read_until(b'\n', &mut payload).await?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&payload)?))
}
