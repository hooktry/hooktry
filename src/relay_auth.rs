use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use rand::RngCore;
use ring::digest::{SHA256, digest};
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
    inner: Arc<RwLock<HashMap<[u8; 32], CapabilityRecord>>>,
}

impl CapabilityStore {
    pub fn issue(&self, exposure_id: Uuid, ttl: Duration) -> RuntimeCapability {
        let token = random_token();
        let expires_at = SystemTime::now() + ttl;
        self.inner
            .write()
            .expect("capability store poisoned")
            .insert(
                token_digest(&token),
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
        let record = store
            .get(&token_digest(token))
            .ok_or(CapabilityError::Invalid)?;
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
        let record = store
            .get_mut(&token_digest(token))
            .ok_or(CapabilityError::Invalid)?;
        record.revoked = true;
        Ok(())
    }
}

pub(crate) fn token_digest(token: &str) -> [u8; 32] {
    let digest = digest(&SHA256, token.as_bytes());
    digest
        .as_ref()
        .try_into()
        .expect("SHA-256 digest is always 32 bytes")
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("write to String cannot fail");
    }

    format!("ortyo_rt_{encoded}")
}
