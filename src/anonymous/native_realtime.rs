use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;
use uuid::Uuid;

use super::ports::{
    AnonymousInteractionStream, AnonymousInteractionSubscription, InteractionStreamError,
    PortFuture, StoredInteraction,
};

#[derive(Clone, Default)]
pub(crate) struct TokioAnonymousInteractionStream {
    viewers: Arc<Mutex<HashMap<Uuid, broadcast::Sender<StoredInteraction>>>>,
}

impl TokioAnonymousInteractionStream {
    fn sender(&self, exposure_id: Uuid) -> broadcast::Sender<StoredInteraction> {
        let mut viewers = self.viewers.lock().expect("anonymous viewers poisoned");
        viewers
            .entry(exposure_id)
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

impl AnonymousInteractionStream for TokioAnonymousInteractionStream {
    fn publish(&self, interaction: StoredInteraction) {
        let _ = self.sender(interaction.exposure_id).send(interaction);
    }

    fn subscribe(
        &self,
        exposure_id: Uuid,
    ) -> Box<dyn AnonymousInteractionSubscription> {
        Box::new(TokioAnonymousInteractionSubscription {
            receiver: self.sender(exposure_id).subscribe(),
        })
    }
}

struct TokioAnonymousInteractionSubscription {
    receiver: broadcast::Receiver<StoredInteraction>,
}

impl AnonymousInteractionSubscription for TokioAnonymousInteractionSubscription {
    fn recv(
        &mut self,
    ) -> PortFuture<'_, Result<StoredInteraction, InteractionStreamError>> {
        Box::pin(async move {
            self.receiver.recv().await.map_err(|error| match error {
                broadcast::error::RecvError::Lagged(_) => InteractionStreamError::Lagged,
                broadcast::error::RecvError::Closed => InteractionStreamError::Closed,
            })
        })
    }
}
