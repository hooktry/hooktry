use std::future::Future;
use std::pin::Pin;

use uuid::Uuid;

use super::{AnonymousError, AnonymousExposureSummary};

pub(crate) type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone)]
pub(crate) struct StoredInteraction {
    pub(crate) interaction_id: Uuid,
    pub(crate) exposure_id: Uuid,
    pub(crate) sequence: u32,
    pub(crate) received_at_unix_ms: u64,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct CreateAnonymousExposure {
    pub(crate) principal_digest: [u8; 32],
    pub(crate) ingress_capability_digest: [u8; 32],
    pub(crate) view_capability_digest: [u8; 32],
    pub(crate) claim_capability_digest: [u8; 32],
    pub(crate) exposure_id: Uuid,
    pub(crate) now: u64,
    pub(crate) expires_at: u64,
}

#[derive(Clone)]
pub(crate) struct CaptureAnonymousInteraction {
    pub(crate) ingress_capability_digest: [u8; 32],
    pub(crate) received_at_ms: u64,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

pub(crate) trait AnonymousExposureRepository: Send + Sync {
    fn purge_expired(&self) -> PortFuture<'_, Result<(), AnonymousError>>;

    fn create(
        &self,
        input: CreateAnonymousExposure,
    ) -> PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>>;

    fn capture(
        &self,
        input: CaptureAnonymousInteraction,
    ) -> PortFuture<'_, Result<StoredInteraction, AnonymousError>>;

    fn view(
        &self,
        view_capability_digest: [u8; 32],
        now: u64,
    ) -> PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>>;

    fn interactions(
        &self,
        exposure_id: Uuid,
    ) -> PortFuture<'_, Result<Vec<StoredInteraction>, AnonymousError>>;

    fn claim(
        &self,
        claim_capability_digest: [u8; 32],
        workspace_id: Uuid,
        now: u64,
    ) -> PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionStreamError {
    Lagged,
    Closed,
}

pub(crate) trait AnonymousInteractionSubscription: Send {
    fn recv(&mut self) -> PortFuture<'_, Result<StoredInteraction, InteractionStreamError>>;
}

pub(crate) trait AnonymousInteractionStream: Send + Sync {
    fn publish(&self, interaction: StoredInteraction);

    fn subscribe(&self, exposure_id: Uuid) -> Box<dyn AnonymousInteractionSubscription>;
}
