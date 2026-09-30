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
    NotFound,
    Inactive,
    Provider(String),
}

pub trait ExposureProvider: Send + Sync {
    fn provision(&self, id: Uuid, target: &ExposureTarget) -> Result<String, ExposureError>;
    fn revoke(&self, exposure: &Exposure) -> Result<(), ExposureError>;
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

#[derive(Clone)]
pub struct ExposureService {
    inner: Arc<RwLock<Vec<Exposure>>>,
    provider: Arc<dyn ExposureProvider>,
}

impl Default for ExposureService {
    fn default() -> Self {
        Self::new(Arc::new(LocalExposureProvider::new(
            "http://127.0.0.1:7777",
        )))
    }
}

impl ExposureService {
    pub fn new(provider: Arc<dyn ExposureProvider>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
            provider,
        }
    }

    pub fn create(
        &self,
        session_id: Uuid,
        name: impl Into<String>,
        port: u16,
    ) -> Result<Exposure, ExposureError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ExposureError::InvalidName);
        }
        if port == 0 {
            return Err(ExposureError::InvalidPort);
        }

        let mut exposures = self.inner.write().expect("exposure store poisoned");
        if exposures
            .iter()
            .any(|item| item.name == name && item.state == ExposureState::Active)
        {
            return Err(ExposureError::DuplicateName);
        }

        let id = Uuid::now_v7();
        let target = ExposureTarget {
            host: "127.0.0.1".to_owned(),
            port,
        };
        let url = self.provider.provision(id, &target)?;
        let exposure = Exposure {
            id,
            session_id,
            name,
            protocol: Protocol::Http,
            mode: ExposureMode::Forward,
            access: ExposureAccess::Private,
            target,
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

        self.provider.revoke(exposure)?;
        exposure.state = ExposureState::Revoked;
        exposure.revoked_at = Some(Utc::now());
        Ok(exposure.clone())
    }
}
