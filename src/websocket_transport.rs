use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::extract::ws::{Message as AxumMessage, WebSocket};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{net::TcpStream, sync::{Mutex, oneshot}};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Message as TungsteniteMessage,
        client::IntoClientRequest,
        http::{HeaderValue, header::AUTHORIZATION},
    },
};
use uuid::Uuid;

use crate::{
    http::{AppState, proxy_relay_request},
    relay::{RelayBroker, RelayResponse},
    relay_transport::RelayFrame,
};

type ClientSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub enum WebSocketTransportError {
    Json(serde_json::Error),
    Transport(String),
    Protocol(String),
}

impl From<serde_json::Error> for WebSocketTransportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub async fn serve_websocket(
    socket: WebSocket,
    broker: RelayBroker,
    exposure_id: Uuid,
) -> Result<(), WebSocketTransportError> {
    let (writer, mut reader) = socket.split();
    let writer = Arc::new(Mutex::new(writer));
    let mut runtime = broker.register(exposure_id).await;
    let registration_id = runtime.registration_id();

    send_axum_frame(&writer, RelayFrame::Registered { exposure_id }).await?;

    let pending = Arc::new(Mutex::new(
        HashMap::<Uuid, oneshot::Sender<RelayResponse>>::new(),
    ));

    let result = async {
        loop {
            tokio::select! {
                incoming = reader.next() => {
                    match incoming {
                        Some(Ok(AxumMessage::Text(text))) => {
                            handle_server_frame(
                                serde_json::from_str(text.as_str())?,
                                &writer,
                                &pending,
                            ).await?;
                        }
                        Some(Ok(AxumMessage::Binary(data))) => {
                            handle_server_frame(
                                serde_json::from_slice(&data)?,
                                &writer,
                                &pending,
                            ).await?;
                        }
                        Some(Ok(AxumMessage::Ping(data))) => {
                            writer
                                .lock()
                                .await
                                .send(AxumMessage::Pong(data))
                                .await
                                .map_err(|error| WebSocketTransportError::Transport(error.to_string()))?;
                        }
                        Some(Ok(AxumMessage::Pong(_))) => {}
                        Some(Ok(AxumMessage::Close(_))) | None => return Ok(()),
                        Some(Err(error)) => {
                            return Err(WebSocketTransportError::Transport(error.to_string()));
                        }
                    }
                }
                work = runtime.recv() => {
                    let Some(work) = work else {
                        return Ok(());
                    };
                    let request_id = work.request.id;
                    let (response_tx, response_rx) = oneshot::channel();
                    pending.lock().await.insert(request_id, response_tx);

                    send_axum_frame(
                        &writer,
                        RelayFrame::Request {
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
            }
        }
    }
    .await;

    pending.lock().await.clear();
    broker.unregister(exposure_id, registration_id).await;
    result
}

async fn handle_server_frame<S>(
    frame: RelayFrame,
    writer: &Arc<Mutex<S>>,
    pending: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<RelayResponse>>>>,
) -> Result<(), WebSocketTransportError>
where
    S: futures_util::Sink<AxumMessage, Error = axum::Error> + Unpin,
{
    match frame {
        RelayFrame::Response { response } => {
            if let Some(tx) = pending.lock().await.remove(&response.request_id) {
                let _ = tx.send(response);
            }
        }
        RelayFrame::Ping { nonce } => {
            send_axum_frame(writer, RelayFrame::Pong { nonce }).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn send_axum_frame<S>(
    writer: &Arc<Mutex<S>>,
    frame: RelayFrame,
) -> Result<(), WebSocketTransportError>
where
    S: futures_util::Sink<AxumMessage, Error = axum::Error> + Unpin,
{
    let payload = serde_json::to_string(&frame)?;
    writer
        .lock()
        .await
        .send(AxumMessage::Text(payload.into()))
        .await
        .map_err(|error| WebSocketTransportError::Transport(error.to_string()))
}

pub struct ConnectedWebSocketRuntime {
    reader: SplitStream<ClientSocket>,
    writer: Arc<Mutex<SplitSink<ClientSocket, TungsteniteMessage>>>,
    state: AppState,
}

pub async fn connect_websocket_runtime(
    runtime_url: &str,
    exposure_id: Uuid,
    capability: &str,
    state: AppState,
) -> Result<ConnectedWebSocketRuntime, WebSocketTransportError> {
    let mut request = runtime_url
        .into_client_request()
        .map_err(|error| WebSocketTransportError::Transport(error.to_string()))?;
    request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {capability}"))
            .map_err(|error| WebSocketTransportError::Transport(error.to_string()))?,
    );

    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| WebSocketTransportError::Transport(error.to_string()))?;
    let (writer, mut reader) = socket.split();
    let writer = Arc::new(Mutex::new(writer));

    match reader.next().await {
        Some(Ok(TungsteniteMessage::Text(text))) => {
            let frame: RelayFrame = serde_json::from_str(text.as_str())?;
            match frame {
                RelayFrame::Registered {
                    exposure_id: registered,
                } if registered == exposure_id => {}
                frame => {
                    return Err(WebSocketTransportError::Protocol(format!(
                        "unexpected registration response: {frame:?}"
                    )));
                }
            }
        }
        Some(Ok(message)) => {
            return Err(WebSocketTransportError::Protocol(format!(
                "unexpected registration message: {message:?}"
            )));
        }
        Some(Err(error)) => {
            return Err(WebSocketTransportError::Transport(error.to_string()));
        }
        None => {
            return Err(WebSocketTransportError::Protocol(
                "relay disconnected before registration".to_owned(),
            ));
        }
    }

    Ok(ConnectedWebSocketRuntime {
        reader,
        writer,
        state,
    })
}

impl ConnectedWebSocketRuntime {
    pub async fn run(mut self) -> Result<(), WebSocketTransportError> {
        while let Some(message) = self.reader.next().await {
            match message.map_err(|error| WebSocketTransportError::Transport(error.to_string()))? {
                TungsteniteMessage::Text(text) => {
                    handle_runtime_frame(
                        serde_json::from_str(text.as_str())?,
                        &self.writer,
                        &self.state,
                    )
                    .await?;
                }
                TungsteniteMessage::Binary(data) => {
                    handle_runtime_frame(
                        serde_json::from_slice(&data)?,
                        &self.writer,
                        &self.state,
                    )
                    .await?;
                }
                TungsteniteMessage::Ping(data) => {
                    self.writer
                        .lock()
                        .await
                        .send(TungsteniteMessage::Pong(data))
                        .await
                        .map_err(|error| WebSocketTransportError::Transport(error.to_string()))?;
                }
                TungsteniteMessage::Pong(_) => {}
                TungsteniteMessage::Close(_) => break,
                TungsteniteMessage::Frame(_) => {}
            }
        }

        Ok(())
    }
}

pub async fn run_websocket_runtime(
    runtime_url: &str,
    exposure_id: Uuid,
    capability: &str,
    state: AppState,
) -> Result<(), WebSocketTransportError> {
    connect_websocket_runtime(runtime_url, exposure_id, capability, state)
        .await?
        .run()
        .await
}

pub async fn maintain_websocket_runtime(
    initial: ConnectedWebSocketRuntime,
    runtime_url: String,
    exposure_id: Uuid,
    capability: String,
    state: AppState,
    retry_delay: Duration,
) {
    let mut connection = Some(initial);
    let mut delay = retry_delay;

    loop {
        if let Some(current) = connection.take() {
            let _ = current.run().await;
        }

        tokio::time::sleep(delay).await;

        match connect_websocket_runtime(
            &runtime_url,
            exposure_id,
            &capability,
            state.clone(),
        )
        .await
        {
            Ok(next) => {
                connection = Some(next);
                delay = retry_delay;
            }
            Err(_) => {
                delay = delay.saturating_mul(2).min(Duration::from_secs(30));
            }
        }
    }
}

async fn handle_runtime_frame<S>(
    frame: RelayFrame,
    writer: &Arc<Mutex<S>>,
    state: &AppState,
) -> Result<(), WebSocketTransportError>
where
    S: futures_util::Sink<TungsteniteMessage, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin
        + Send
        + 'static,
{
    match frame {
        RelayFrame::Request { request } => {
            let writer = writer.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let response = match proxy_relay_request(&state, &request).await {
                    Ok(response) => response,
                    Err(status) => RelayResponse {
                        request_id: request.id,
                        status: status.as_u16(),
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                };
                let _ = send_tungstenite_frame(&writer, RelayFrame::Response { response }).await;
            });
        }
        RelayFrame::Ping { nonce } => {
            send_tungstenite_frame(writer, RelayFrame::Pong { nonce }).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn send_tungstenite_frame<S>(
    writer: &Arc<Mutex<S>>,
    frame: RelayFrame,
) -> Result<(), WebSocketTransportError>
where
    S: futures_util::Sink<TungsteniteMessage, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let payload = serde_json::to_string(&frame)?;
    writer
        .lock()
        .await
        .send(TungsteniteMessage::Text(payload.into()))
        .await
        .map_err(|error| WebSocketTransportError::Transport(error.to_string()))
}
