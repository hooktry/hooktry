use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCapability {
    pub token: String,
    pub exposure_id: Uuid,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    Invalid,
    Expired,
    Revoked,
    WrongExposure,
}

#[derive(Debug, Clone)]
struct CapabilityRecord {
    exposure_id: Uuid,
    expires_at: SystemTime,
    revoked: bool,
}

#[derive(Clone, Default)]
pub struct CapabilityStore {
    inner: Arc<RwLock<HashMap<String, CapabilityRecord>>>,
}

impl CapabilityStore {
    pub fn issue(&self, exposure_id: Uuid, ttl: Duration) -> RuntimeCapability {
        let token = format!("ortyo_rt_{}", Uuid::now_v7().simple());
        let expires_at = SystemTime::now() + ttl;
        self.inner
            .write()
            .expect("capability store poisoned")
            .insert(
                token.clone(),
                CapabilityRecord {
                    exposure_id,
                    expires_at,
                    revoked: false,
                },
            );
        RuntimeCapability {
            token,
            exposure_id,
            expires_at,
        }
    }

    pub fn authorize(&self, exposure_id: Uuid, token: &str) -> Result<(), CapabilityError> {
        let store = self.inner.read().expect("capability store poisoned");
        let record = store.get(token).ok_or(CapabilityError::Invalid)?;
        if record.revoked {
            return Err(CapabilityError::Revoked);
        }
        if record.exposure_id != exposure_id {
            return Err(CapabilityError::WrongExposure);
        }
        if SystemTime::now() >= record.expires_at {
            return Err(CapabilityError::Expired);
        }
        Ok(())
    }

    pub fn revoke(&self, token: &str) -> Result<(), CapabilityError> {
        let mut store = self.inner.write().expect("capability store poisoned");
        let record = store.get_mut(token).ok_or(CapabilityError::Invalid)?;
        record.revoked = true;
        Ok(())
    }
}
