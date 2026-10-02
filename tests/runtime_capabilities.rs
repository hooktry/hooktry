use std::time::Duration;

use hooktry::{
    relay::RelayBroker,
    relay_auth::{CapabilityError, CapabilityStore},
    relay_transport::{RelayFrame, serve_connection},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use uuid::Uuid;

#[test]
fn capability_is_scoped_expires_and_can_be_revoked() {
    let store = CapabilityStore::default();
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();

    let capability = store.issue(first, Duration::from_secs(60)).unwrap();
    let second_capability = store.issue(first, Duration::from_secs(60)).unwrap();
    assert!(capability.token.starts_with("hooktry_rt_"));
    assert_eq!(capability.token.len(), "hooktry_rt_".len() + 64);
    assert_ne!(capability.token, second_capability.token);
    assert_eq!(store.authorize(first, &capability.token), Ok(()));
    assert_eq!(
        store.authorize(second, &capability.token),
        Err(CapabilityError::WrongExposure)
    );

    store.revoke(&capability.token).unwrap();
    assert_eq!(
        store.authorize(first, &capability.token),
        Err(CapabilityError::Revoked)
    );

    let expired = store.issue(first, Duration::ZERO).unwrap();
    assert_eq!(
        store.authorize(first, &expired.token),
        Err(CapabilityError::Expired)
    );
}

#[test]
fn rotating_exposure_capability_revokes_previous_tokens() {
    let store = CapabilityStore::default();
    let exposure_id = Uuid::now_v7();

    let first = store.issue(exposure_id, Duration::from_secs(60)).unwrap();
    let second = store
        .rotate_exposure(exposure_id, Duration::from_secs(60))
        .unwrap();

    assert_ne!(first.token, second.token);
    assert_eq!(first.exposure_id, second.exposure_id);
    assert_eq!(
        store.authorize(exposure_id, &first.token),
        Err(CapabilityError::Revoked)
    );
    assert_eq!(store.authorize(exposure_id, &second.token), Ok(()));
}

#[tokio::test]
async fn relay_rejects_registration_without_valid_capability() {
    let broker = RelayBroker::default();
    let capabilities = CapabilityStore::default();
    let exposure_id = Uuid::now_v7();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        serve_connection(stream, broker, capabilities).await
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let frame = RelayFrame::Register {
        exposure_id,
        capability: "hooktry_rt_invalid".to_owned(),
    };
    let mut payload = serde_json::to_vec(&frame).unwrap();
    payload.push(b'\n');
    stream.write_all(&payload).await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = reader.read_line(&mut line).await.unwrap();
    assert_eq!(read, 0);

    let result = server.await.unwrap();
    assert!(result.is_err());
}
