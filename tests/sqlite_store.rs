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

    std::fs::remove_file(path).unwrap();
}
