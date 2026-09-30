use std::sync::{Arc, RwLock};

use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    Exposure, ExposureAccess, ExposureMode, ExposureState, ExposureTarget, Protocol,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExposureError {
    InvalidName,
    InvalidPort,
    DuplicateName,
    DuplicateId,
    NotFound,
    Inactive,
    Provider(String),
}

pub trait ExposureProvider: Send + Sync {
    fn provision(&self, id: Uuid, target: &ExposureTarget) -> Result<String, ExposureError>;
    fn revoke(&self, exposure: &Exposure) -> Result<(), ExposureError>;
}

#[derive(Debug, Clone)]
pub struct CreateExposure {
    pub name: String,
    pub target: ExposureTarget,
    pub mode: ExposureMode,
    pub access: ExposureAccess,
}

impl CreateExposure {
    pub fn forward(name: impl Into<String>, port: u16) -> Self {
        Self {
            name: name.into(),
            target: ExposureTarget {
                host: "127.0.0.1".to_owned(),
                port,
            },
            mode: ExposureMode::Forward,
            access: ExposureAccess::Private,
        }
    }
}

#[derive(Debug)]
pub struct LocalExposureProvider {
    base_url: String,
}

impl LocalExposureProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }
}

impl ExposureProvider for LocalExposureProvider {
    fn provision(&self, id: Uuid, _target: &ExposureTarget) -> Result<String, ExposureError> {
        Ok(format!("{}/exposed/{id}", self.base_url))
    }

    fn revoke(&self, _exposure: &Exposure) -> Result<(), ExposureError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct RelayExposureProvider {
    base_url: String,
}

impl RelayExposureProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }
}

impl ExposureProvider for RelayExposureProvider {
    fn provision(&self, id: Uuid, _target: &ExposureTarget) -> Result<String, ExposureError> {
        Ok(format!("{}/e/{id}", self.base_url))
    }

    fn revoke(&self, _exposure: &Exposure) -> Result<(), ExposureError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ExposureService {
    inner: Arc<RwLock<Vec<Exposure>>>,
    forward_provider: Arc<dyn ExposureProvider>,
    relay_provider: Arc<dyn ExposureProvider>,
}

impl Default for ExposureService {
    fn default() -> Self {
        Self::with_providers(
            Arc::new(LocalExposureProvider::new("http://127.0.0.1:7777")),
            Arc::new(RelayExposureProvider::new("https://relay.ortyo.test")),
        )
    }
}

impl ExposureService {
    pub fn new(provider: Arc<dyn ExposureProvider>) -> Self {
        Self::with_providers(provider.clone(), provider)
    }

    pub fn with_providers(
        forward_provider: Arc<dyn ExposureProvider>,
        relay_provider: Arc<dyn ExposureProvider>,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
            forward_provider,
            relay_provider,
        }
    }

    pub fn create(
        &self,
        session_id: Uuid,
        name: impl Into<String>,
        port: u16,
    ) -> Result<Exposure, ExposureError> {
        self.create_with(session_id, CreateExposure::forward(name, port))
    }

    pub fn create_with(
        &self,
        session_id: Uuid,
        request: CreateExposure,
    ) -> Result<Exposure, ExposureError> {
        self.create_with_id(session_id, Uuid::now_v7(), request)
    }

    pub fn create_with_id(
        &self,
        session_id: Uuid,
        id: Uuid,
        request: CreateExposure,
    ) -> Result<Exposure, ExposureError> {
        if request.name.trim().is_empty() {
            return Err(ExposureError::InvalidName);
        }
        if request.target.port == 0 {
            return Err(ExposureError::InvalidPort);
        }

        let mut exposures = self.inner.write().expect("exposure store poisoned");
        if exposures
            .iter()
            .any(|item| item.name == request.name && item.state == ExposureState::Active)
        {
            return Err(ExposureError::DuplicateName);
        }

        if exposures.iter().any(|item| item.id == id) {
            return Err(ExposureError::DuplicateId);
        }

        let provider = match request.mode {
            ExposureMode::Forward => &self.forward_provider,
            ExposureMode::Relay => &self.relay_provider,
        };
        let url = provider.provision(id, &request.target)?;
        let exposure = Exposure {
            id,
            session_id,
            name: request.name,
            protocol: Protocol::Http,
            mode: request.mode,
            access: request.access,
            target: request.target,
            url,
            state: ExposureState::Active,
            created_at: Utc::now(),
            revoked_at: None,
        };
        exposures.push(exposure.clone());
        Ok(exposure)
    }

    pub fn all(&self) -> Vec<Exposure> {
        self.inner.read().expect("exposure store poisoned").clone()
    }

    pub fn get(&self, id: Uuid) -> Option<Exposure> {
        self.inner
            .read()
            .expect("exposure store poisoned")
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    pub fn active(&self, id: Uuid) -> Result<Exposure, ExposureError> {
        let exposure = self.get(id).ok_or(ExposureError::NotFound)?;
        if exposure.state != ExposureState::Active {
            return Err(ExposureError::Inactive);
        }
        Ok(exposure)
    }

    pub fn revoke(&self, id: Uuid) -> Result<Exposure, ExposureError> {
        let mut exposures = self.inner.write().expect("exposure store poisoned");
        let exposure = exposures
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or(ExposureError::NotFound)?;

        if exposure.state != ExposureState::Active {
            return Err(ExposureError::Inactive);
        }

        let provider = match exposure.mode {
            ExposureMode::Forward => &self.forward_provider,
            ExposureMode::Relay => &self.relay_provider,
        };
        provider.revoke(exposure)?;
        exposure.state = ExposureState::Revoked;
        exposure.revoked_at = Some(Utc::now());
        Ok(exposure.clone())
    }
}
