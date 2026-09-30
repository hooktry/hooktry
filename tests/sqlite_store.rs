use chrono::Utc;
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
