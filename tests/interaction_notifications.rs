use chrono::Utc;
use ortyo::{
    domain::{Direction, Interaction, Origin, Protocol},
    store::InteractionStore,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn interaction_revision_wakes_after_persisted_evidence() {
    let store = InteractionStore::default();
    let mut revision = store.subscribe_interactions();
    let before = *revision.borrow();

    store.record(Interaction {
        id: Uuid::now_v7(),
        session_id: Uuid::now_v7(),
        protocol: Protocol::Http,
        direction: Direction::Inbound,
        origin: Origin::Observed,
        operation: "POST /webhook".into(),
        started_at: Utc::now(),
        duration_ms: 1,
        observed_sequence: None,
        request: json!({}),
        response: json!({"status": 200}),
        source_interaction_id: None,
        context: Default::default(),
    });

    revision.changed().await.unwrap();

    assert_ne!(*revision.borrow(), before);
    assert_eq!(store.all().len(), 1);
}
