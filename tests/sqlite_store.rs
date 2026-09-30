use chrono::{Duration, Utc};
use ortyo::{
    domain::{Direction, Interaction, Origin, Protocol},
    store::InteractionStore,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn interaction_survives_database_reopen() {
    let path = std::env::temp_dir().join(format!("ortyo-{}.db", Uuid::now_v7()));
    let session_id = Uuid::now_v7();
    let interaction_id = Uuid::now_v7();

    {
        let store = InteractionStore::open(&path).unwrap();
        store.record(Interaction {
            id: interaction_id,
            session_id,
            protocol: Protocol::Http,
            direction: Direction::Inbound,
            origin: Origin::Observed,
            operation: "POST /stripe/payment_intents".into(),
            started_at: Utc::now(),
            duration_ms: 3,
            observed_sequence: None,
            request: json!({"body": {"amount": 4999}}),
            response: json!({"status": 200}),
            source_interaction_id: None,
            context: Default::default(),
        });
    }

    let reopened = InteractionStore::open(&path).unwrap();
    let interactions = reopened.all();

    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].id, interaction_id);
    assert_eq!(interactions[0].session_id, session_id);
    assert_eq!(interactions[0].operation, "POST /stripe/payment_intents");
    assert_eq!(interactions[0].observed_sequence, Some(1));

    std::fs::remove_file(path).unwrap();
}

#[test]
fn persistence_order_is_durable_and_independent_from_interaction_timestamps() {
    let path = std::env::temp_dir().join(format!("ortyo-order-{}.db", Uuid::now_v7()));
    let session_id = Uuid::now_v7();
    let later_clock = Utc::now();
    let earlier_clock = later_clock - Duration::seconds(10);
    let first_id = Uuid::now_v7();
    let second_id = Uuid::now_v7();

    {
        let store = InteractionStore::open(&path).unwrap();
        store.record(Interaction {
            id: first_id,
            session_id,
            protocol: Protocol::Http,
            direction: Direction::Inbound,
            origin: Origin::Observed,
            operation: "POST /first".into(),
            started_at: later_clock,
            duration_ms: 1,
            observed_sequence: None,
            request: json!({}),
            response: json!({"status": 200}),
            source_interaction_id: None,
            context: Default::default(),
        });

        store.record(Interaction {
            id: second_id,
            session_id,
            protocol: Protocol::Http,
            direction: Direction::Inbound,
            origin: Origin::Observed,
            operation: "POST /second".into(),
            started_at: earlier_clock,
            duration_ms: 1,
            observed_sequence: None,
            request: json!({}),
            response: json!({"status": 200}),
            source_interaction_id: None,
            context: Default::default(),
        });

        let timestamp_order = store.all();
        assert_eq!(timestamp_order[0].id, second_id);
    }

    let reopened = InteractionStore::open(&path).unwrap();
    let persistence_order = reopened.all_recorded();
    assert_eq!(
        persistence_order
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    assert_eq!(persistence_order[0].observed_sequence, Some(1));
    assert_eq!(persistence_order[1].observed_sequence, Some(2));

    std::fs::remove_file(path).unwrap();
}

#[test]
fn legacy_interaction_payload_is_backfilled_with_observed_sequence_on_read() {
    let path = std::env::temp_dir().join(format!("ortyo-legacy-order-{}.db", Uuid::now_v7()));
    let interaction_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let started_at = Utc::now();
    let payload = json!({
        "id": interaction_id,
        "session_id": session_id,
        "protocol": "http",
        "direction": "inbound",
        "origin": "observed",
        "operation": "POST /legacy",
        "started_at": started_at,
        "duration_ms": 1,
        "request": {},
        "response": {}
    })
    .to_string();

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE interactions (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    payload TEXT NOT NULL
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO interactions (id, session_id, started_at, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    interaction_id.to_string(),
                    session_id.to_string(),
                    started_at.to_rfc3339(),
                    payload
                ],
            )
            .unwrap();
    }

    let store = InteractionStore::open(&path).unwrap();
    let interaction = store.find(interaction_id).unwrap();

    assert_eq!(interaction.observed_sequence, Some(1));
    assert_eq!(interaction.operation, "POST /legacy");

    std::fs::remove_file(path).unwrap();
}
