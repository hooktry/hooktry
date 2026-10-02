use std::time::Duration;

use hooktry::relay::{RelayBroker, RelayError, RelayRequest};
use uuid::Uuid;

#[tokio::test]
async fn unregister_removes_only_the_registration_that_owns_the_slot() {
    let broker = RelayBroker::default();
    let exposure_id = Uuid::now_v7();

    let first = broker.register(exposure_id).await;
    let first_id = first.registration_id();
    let mut second = broker.register(exposure_id).await;
    let second_id = second.registration_id();

    broker.unregister(exposure_id, first_id).await;

    let request = request_for(exposure_id);
    let request_id = request.id;
    let ingress = broker.ingress_with_timeout(request, Duration::from_secs(1));
    let worker = async {
        let work = second.recv().await.unwrap();
        work.complete(hooktry::relay::RelayResponse {
            request_id,
            status: 204,
            headers: Vec::new(),
            body: Vec::new(),
        })
        .unwrap();
    };
    let (response, ()) = tokio::join!(ingress, worker);
    assert_eq!(response.unwrap().status, 204);

    broker.unregister(exposure_id, second_id).await;
    assert_eq!(
        broker.ingress(request_for(exposure_id)).await.unwrap_err(),
        RelayError::RuntimeUnavailable
    );
}

#[tokio::test]
async fn full_runtime_queue_fails_fast_with_overloaded() {
    let broker = RelayBroker::default();
    let exposure_id = Uuid::now_v7();
    let _runtime = broker.register(exposure_id).await;

    let mut pending = Vec::new();
    for _ in 0..16 {
        let broker = broker.clone();
        pending.push(tokio::spawn(async move {
            broker
                .ingress_with_timeout(request_for(exposure_id), Duration::from_secs(5))
                .await
        }));
    }

    tokio::time::sleep(Duration::from_millis(25)).await;
    let overloaded = broker
        .ingress_with_timeout(request_for(exposure_id), Duration::from_millis(50))
        .await
        .unwrap_err();
    assert_eq!(overloaded, RelayError::Overloaded);

    for task in pending {
        task.abort();
    }
}

fn request_for(exposure_id: Uuid) -> RelayRequest {
    RelayRequest {
        id: Uuid::now_v7(),
        exposure_id,
        method: "GET".to_owned(),
        path: "/".to_owned(),
        query: None,
        headers: Vec::new(),
        body: Vec::new(),
    }
}
