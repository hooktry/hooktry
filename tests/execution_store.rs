use std::{collections::BTreeMap, fs};

use ortyo::{
    execution::{
        ExecutionError, ExecutionOutcome, ExecutionProviderKind, ExecutionRecord,
        HttpExecutionRequest,
    },
    execution_store::{DurableExecutionState, ExecutionStore, ExecutionStoreError},
};
use uuid::Uuid;

#[test]
fn execution_store_persists_started_then_completed_lifecycle() {
    let path = std::env::temp_dir().join(format!("ortyo-execution-{}.db", Uuid::now_v7()));
    let workspace_id = Uuid::now_v7();
    let execution_id = Uuid::now_v7();

    let store = ExecutionStore::open(&path).unwrap();
    let started = store
        .reserve(workspace_id, execution_id, ExecutionProviderKind::Http)
        .unwrap();
    assert_eq!(started.state, DurableExecutionState::Started);
    assert_eq!(started.execution_id, execution_id);
    assert_eq!(started.workspace_id, workspace_id);
    assert_eq!(started.completed_at_unix_ms, None);
    assert_eq!(started.outcome, None);

    drop(store);
    let store = ExecutionStore::open(&path).unwrap();
    let reopened = store.get(workspace_id, execution_id).unwrap().unwrap();
    assert_eq!(reopened, started);

    let terminal = ExecutionRecord {
        execution_id,
        workspace_id,
        provider: ExecutionProviderKind::Http,
        started_at_unix_ms: started.started_at_unix_ms,
        completed_at_unix_ms: started.started_at_unix_ms + 7,
        outcome: ExecutionOutcome::Rejected {
            error: ExecutionError::UnsafeDestination,
        },
    };
    let completed = store.complete(&terminal).unwrap();
    assert_eq!(completed.state, DurableExecutionState::Completed);
    assert_eq!(completed.terminal_record(), Some(terminal.clone()));
    assert_eq!(
        store.complete(&terminal),
        Err(ExecutionStoreError::AlreadyCompleted)
    );

    drop(store);
    let reopened = ExecutionStore::open(&path).unwrap();
    assert_eq!(
        reopened
            .get(workspace_id, execution_id)
            .unwrap()
            .unwrap()
            .terminal_record(),
        Some(terminal)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn execution_store_is_workspace_scoped_and_discards_only_uncompleted_reservations() {
    let store = ExecutionStore::default();
    let workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let execution_id = Uuid::now_v7();

    let started = store
        .reserve(workspace_id, execution_id, ExecutionProviderKind::Http)
        .unwrap();

    assert_eq!(store.get(other_workspace_id, execution_id).unwrap(), None);
    store
        .discard_started(other_workspace_id, execution_id)
        .unwrap();
    assert!(store.get(workspace_id, execution_id).unwrap().is_some());

    store.discard_started(workspace_id, execution_id).unwrap();
    assert_eq!(store.get(workspace_id, execution_id).unwrap(), None);

    let completed_id = Uuid::now_v7();
    let started = store
        .reserve(workspace_id, completed_id, ExecutionProviderKind::Http)
        .unwrap();
    let terminal = ExecutionRecord {
        execution_id: completed_id,
        workspace_id,
        provider: ExecutionProviderKind::Http,
        started_at_unix_ms: started.started_at_unix_ms,
        completed_at_unix_ms: started.started_at_unix_ms + 1,
        outcome: ExecutionOutcome::Rejected {
            error: ExecutionError::InvalidRequest,
        },
    };
    store.complete(&terminal).unwrap();
    store.discard_started(workspace_id, completed_id).unwrap();
    assert!(store.get(workspace_id, completed_id).unwrap().is_some());
}

#[test]
fn durable_store_does_not_persist_execution_request_material() {
    let path = std::env::temp_dir().join(format!("ortyo-execution-{}.db", Uuid::now_v7()));
    let workspace_id = Uuid::now_v7();
    let execution_id = Uuid::now_v7();
    let store = ExecutionStore::open(&path).unwrap();

    let started = store
        .reserve(workspace_id, execution_id, ExecutionProviderKind::Http)
        .unwrap();

    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url: "https://api.example.com/run?token=query-secret".to_owned(),
        headers: BTreeMap::from([("x-api-key".to_owned(), "header-secret".to_owned())]),
        body: Some(serde_json::json!({"secret":"body-secret"})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 1000,
    };
    let serialized_request = serde_json::to_string(&request).unwrap();
    assert!(serialized_request.contains("query-secret"));
    assert!(serialized_request.contains("header-secret"));
    assert!(serialized_request.contains("body-secret"));

    drop(store);
    let bytes = fs::read(&path).unwrap();
    for secret in ["query-secret", "header-secret", "body-secret"] {
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "execution store persisted request material: {secret}"
        );
    }

    let reopened = ExecutionStore::open(&path).unwrap();
    assert_eq!(
        reopened
            .get(workspace_id, execution_id)
            .unwrap()
            .unwrap()
            .started_at_unix_ms,
        started.started_at_unix_ms
    );

    let _ = fs::remove_file(path);
}
