use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{Interaction, Origin, Recording},
    store::InteractionStore,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    RecordingNotFound,
    SourceInteractionNotFound(Uuid),
}

pub fn snapshot(store: &InteractionStore, interaction_ids: Vec<Uuid>) -> Recording {
    let recording = Recording {
        id: Uuid::now_v7(),
        created_at: Utc::now(),
        interaction_ids,
    };
    store.save_recording(&recording);
    recording
}

pub fn snapshot_all(store: &InteractionStore) -> Recording {
    snapshot(store, store.all().into_iter().map(|item| item.id).collect())
}

pub fn replay(
    store: &InteractionStore,
    session_id: Uuid,
    recording_id: Uuid,
) -> Result<Vec<Interaction>, ReplayError> {
    let recording = store
        .recording(recording_id)
        .ok_or(ReplayError::RecordingNotFound)?;
    let mut replayed = Vec::new();

    for source_id in recording.interaction_ids {
        let source = store
            .find(source_id)
            .ok_or(ReplayError::SourceInteractionNotFound(source_id))?;
        let interaction = Interaction {
            id: Uuid::now_v7(),
            session_id,
            protocol: source.protocol,
            direction: source.direction,
            origin: Origin::Replayed,
            operation: source.operation,
            started_at: Utc::now(),
            duration_ms: 0,
            request: source.request,
            response: source.response,
            source_interaction_id: Some(source.id),
            context: source.context,
        };
        store.record(interaction.clone());
        replayed.push(interaction);
    }

    Ok(replayed)
}
