use std::{io, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{Mutex, mpsc},
    time::sleep,
};
use uuid::Uuid;

use crate::{
    http::{AppState, proxy_relay_request},
    relay::{RelayBroker, RelayRequest, RelayResponse},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayFrame {
    Register { exposure_id: Uuid },
    Registered { exposure_id: Uuid },
    Request { request: RelayRequest },
    Response { response: RelayResponse },
    Ping { nonce: Uuid },
    Pong { nonce: Uuid },
    Error { message: String },
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
) -> Result<(), TransportError> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let writer = Arc::new(Mutex::new(write_half));

    let exposure_id = match read_frame::<RelayFrame, _>(&mut reader).await? {
        Some(RelayFrame::Register { exposure_id }) => exposure_id,
        Some(_) => return Err(TransportError::Protocol("expected register frame".to_owned())),
        None => return Ok(()),
    };

    let mut runtime = broker.register(exposure_id).await;
    write_frame(
        &mut *writer.lock().await,
        &RelayFrame::Registered { exposure_id },
    )
    .await?;

    let work_writer = writer.clone();
    let work_task = tokio::spawn(async move {
        while let Some(work) = runtime.recv().await {
            write_frame(
                &mut *work_writer.lock().await,
                &RelayFrame::Request {
                    request: work.request.clone(),
                },
            )
            .await?;
            // ACCESS3 deliberately handles one in-flight work item per runtime connection.
            // Multiplexing the wire is encoded by request id and will be made concurrent in ACCESS4.
            let response = wait_for_response(&mut reader, &writer, work.request.id).await?;
            work.complete(response)
                .map_err(|error| TransportError::Protocol(format!("{error:?}")))?;
        }
        Ok::<(), TransportError>(())
    });

    work_task
        .await
        .map_err(|error| TransportError::Protocol(error.to_string()))?
}

async fn wait_for_response<R>(
    reader: &mut R,
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    request_id: Uuid,
) -> Result<RelayResponse, TransportError>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    loop {
        match read_frame::<RelayFrame, _>(reader).await? {
            Some(RelayFrame::Response { response }) if response.request_id == request_id => {
                return Ok(response);
            }
            Some(RelayFrame::Ping { nonce }) => {
                write_frame(&mut *writer.lock().await, &RelayFrame::Pong { nonce }).await?;
            }
            Some(RelayFrame::Response { response }) => {
                return Err(TransportError::Protocol(format!(
                    "unexpected response {}, waiting for {request_id}",
                    response.request_id
                )));
            }
            Some(_) => {}
            None => {
                return Err(TransportError::Protocol(
                    "runtime disconnected before response".to_owned(),
                ));
            }
        }
    }
}

pub async fn run_runtime_connection(
    stream: TcpStream,
    exposure_id: Uuid,
    state: AppState,
) -> Result<(), TransportError> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    write_frame(&mut write_half, &RelayFrame::Register { exposure_id }).await?;
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
                let response = proxy_relay_request(&state, &request)
                    .await
                    .map_err(|status| TransportError::Protocol(format!("boundary returned {status}")))?;
                write_frame(&mut write_half, &RelayFrame::Response { response }).await?;
            }
            RelayFrame::Ping { nonce } => {
                write_frame(&mut write_half, &RelayFrame::Pong { nonce }).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn run_runtime_reconnecting(
    relay_addr: &str,
    exposure_id: Uuid,
    state: AppState,
    retry_delay: Duration,
) -> Result<(), TransportError> {
    loop {
        match TcpStream::connect(relay_addr).await {
            Ok(stream) => {
                let _ = run_runtime_connection(stream, exposure_id, state.clone()).await;
            }
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                return Err(TransportError::Io(error));
            }
            Err(_) => {}
        }
        sleep(retry_delay).await;
    }
}

pub async fn serve_listener(
    listener: TcpListener,
    broker: RelayBroker,
) -> Result<(), TransportError> {
    loop {
        let (stream, _) = listener.accept().await?;
        let broker = broker.clone();
        tokio::spawn(async move {
            let _ = serve_connection(stream, broker).await;
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
