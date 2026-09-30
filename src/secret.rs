use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    InvalidName,
    NotFound,
}

#[derive(Clone, Default)]
pub struct SecretStore {
    inner: Arc<RwLock<HashMap<(Uuid, String), StoredSecret>>>,
}

#[derive(Clone)]
struct StoredSecret {
    reference: SecretRef,
    value: String,
}

impl SecretStore {
    pub fn put(
        &self,
        workspace_id: Uuid,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<SecretRef, SecretError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SecretError::InvalidName);
        }
        let reference = SecretRef {
            id: Uuid::now_v7(),
            workspace_id,
            name: name.clone(),
        };
        self.inner.write().expect("secret store poisoned").insert(
            (workspace_id, name),
            StoredSecret {
                reference: reference.clone(),
                value: value.into(),
            },
        );
        Ok(reference)
    }

    pub fn resolve(&self, workspace_id: Uuid, name: &str) -> Result<String, SecretError> {
        self.inner
            .read()
            .expect("secret store poisoned")
            .get(&(workspace_id, name.to_owned()))
            .map(|secret| secret.value.clone())
            .ok_or(SecretError::NotFound)
    }

    pub fn get_ref(&self, workspace_id: Uuid, name: &str) -> Option<SecretRef> {
        self.inner
            .read()
            .expect("secret store poisoned")
            .get(&(workspace_id, name.to_owned()))
            .map(|secret| secret.reference.clone())
    }
}
