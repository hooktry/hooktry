use serde::Serialize;

use crate::{
    approval::{
        ApprovalKeyDependencyCount, ApprovalKeyDependencyReport, ApprovalStore,
        derive_digest_keyring,
    },
    keyring::{VersionedKeyring, VersionedKeyringError},
    secret::{SecretKeyVersionCount, SecretRewrapReport, SecretStore, decode_master_key},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMaintenanceConfig {
    pub database_url: Option<String>,
    pub db_path: String,
    pub keyring: VersionedKeyring,
}

impl KeyMaintenanceConfig {
    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, String> {
        let database_url = lookup("ORTYO_DATABASE_URL").filter(|url| !url.trim().is_empty());
        if database_url
            .as_ref()
            .is_some_and(|url| !url.starts_with("postgres://") && !url.starts_with("postgresql://"))
        {
            return Err("ORTYO_DATABASE_URL must be a PostgreSQL URL".to_owned());
        }

        let active_key = lookup("ORTYO_SECRETS_KEY")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "ORTYO_SECRETS_KEY is required".to_owned())
            .and_then(|value| {
                decode_master_key(&value).map_err(|_| {
                    "ORTYO_SECRETS_KEY must be exactly 64 hexadecimal characters".to_owned()
                })
            })?;
        let active_version = lookup("ORTYO_SECRETS_KEY_VERSION")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_positive_key_version("ORTYO_SECRETS_KEY_VERSION", &value))
            .transpose()?
            .unwrap_or(1);
        let previous = lookup("ORTYO_SECRETS_PREVIOUS_KEYS")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_previous_secret_keys(&value))
            .transpose()?
            .unwrap_or_default();
        let keyring =
            VersionedKeyring::new(active_version, active_key, previous).map_err(keyring_error)?;
        let db_path = lookup("ORTYO_HOSTED_DB_PATH")
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| "ortyo-hosted.db".to_owned());

        Ok(Self {
            database_url,
            db_path,
            keyring,
        })
    }
}

#[derive(Clone)]
pub struct KeyMaintenance {
    keyring: VersionedKeyring,
    secrets: SecretStore,
    approvals: ApprovalStore,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeyRetirementReadiness {
    pub key_version: i32,
    pub configured: bool,
    pub active: bool,
    pub encrypted_secrets: u64,
    pub actionable_approvals: u64,
    pub malformed_actionable_approvals: u64,
    pub safe_to_retire: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeyStatusReport {
    pub active_version: i32,
    pub configured_versions: Vec<i32>,
    pub secret_versions: Vec<SecretKeyVersionCount>,
    pub approval_dependencies: Vec<ApprovalKeyDependencyCount>,
    pub malformed_actionable_approvals: u64,
    pub retirement: Vec<KeyRetirementReadiness>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeyRewrapOutcome {
    pub rewrap: SecretRewrapReport,
    pub retirement: KeyRetirementReadiness,
}

impl KeyMaintenance {
    pub fn open(config: KeyMaintenanceConfig) -> Result<Self, String> {
        let keyring = config.keyring;
        let approvals_keyring = derive_digest_keyring(&keyring);
        let (secrets, approvals) = if let Some(database_url) = config.database_url {
            (
                SecretStore::open_postgres_with_keyring(&database_url, keyring.clone())
                    .map_err(|error| format!("open Postgres secret store: {error:?}"))?,
                ApprovalStore::open_postgres_with_digest_keyring(&database_url, approvals_keyring)
                    .map_err(|error| format!("open Postgres approval store: {error:?}"))?,
            )
        } else {
            (
                SecretStore::open_with_keyring(&config.db_path, keyring.clone())
                    .map_err(|error| format!("open SQLite secret store: {error:?}"))?,
                ApprovalStore::open_with_digest_keyring(&config.db_path, approvals_keyring)
                    .map_err(|error| format!("open SQLite approval store: {error:?}"))?,
            )
        };

        Ok(Self {
            keyring,
            secrets,
            approvals,
        })
    }

    pub fn status(&self) -> Result<KeyStatusReport, String> {
        let secret_versions = self
            .secrets
            .key_version_counts()
            .map_err(|error| format!("inspect secret key versions: {error:?}"))?;
        let approval_report = self
            .approvals
            .actionable_key_dependencies()
            .map_err(|error| format!("inspect approval key dependencies: {error:?}"))?;
        let configured_versions = self.keyring.versions().collect::<Vec<_>>();
        let retirement = configured_versions
            .iter()
            .copied()
            .filter(|version| *version != self.keyring.active_version())
            .map(|version| {
                retirement_readiness(&self.keyring, version, &secret_versions, &approval_report)
            })
            .collect();

        Ok(KeyStatusReport {
            active_version: self.keyring.active_version(),
            configured_versions,
            secret_versions,
            approval_dependencies: approval_report.dependencies,
            malformed_actionable_approvals: approval_report.malformed_keyed,
            retirement,
        })
    }

    pub fn rewrap(&self, from_version: i32, apply: bool) -> Result<KeyRewrapOutcome, String> {
        if from_version == self.keyring.active_version() {
            return Err(format!(
                "key version {from_version} is active and cannot be rewrapped or retired"
            ));
        }
        if self.keyring.key(from_version).is_none() {
            return Err(format!(
                "key version {from_version} is not configured in ORTYO_SECRETS_PREVIOUS_KEYS"
            ));
        }

        let rewrap = self
            .secrets
            .rewrap_key_version(from_version, apply)
            .map_err(|error| {
                format!("rewrap secrets from key version {from_version}: {error:?}")
            })?;
        let retirement = self.retire_check(from_version)?;
        Ok(KeyRewrapOutcome { rewrap, retirement })
    }

    pub fn retire_check(&self, key_version: i32) -> Result<KeyRetirementReadiness, String> {
        if key_version <= 0 {
            return Err("key version must be a positive integer".to_owned());
        }
        let secret_versions = self
            .secrets
            .key_version_counts()
            .map_err(|error| format!("inspect secret key versions: {error:?}"))?;
        let approval_report = self
            .approvals
            .actionable_key_dependencies()
            .map_err(|error| format!("inspect approval key dependencies: {error:?}"))?;

        Ok(retirement_readiness(
            &self.keyring,
            key_version,
            &secret_versions,
            &approval_report,
        ))
    }
}

fn retirement_readiness(
    keyring: &VersionedKeyring,
    key_version: i32,
    secret_versions: &[SecretKeyVersionCount],
    approval_report: &ApprovalKeyDependencyReport,
) -> KeyRetirementReadiness {
    let encrypted_secrets = secret_versions
        .iter()
        .find(|entry| entry.key_version == key_version)
        .map(|entry| entry.count)
        .unwrap_or(0);
    let actionable_approvals = approval_report
        .dependencies
        .iter()
        .find(|entry| entry.key_version == key_version)
        .map(|entry| entry.total)
        .unwrap_or(0);
    let active = key_version == keyring.active_version();
    let malformed_actionable_approvals = approval_report.malformed_keyed;

    KeyRetirementReadiness {
        key_version,
        configured: keyring.key(key_version).is_some(),
        active,
        encrypted_secrets,
        actionable_approvals,
        malformed_actionable_approvals,
        safe_to_retire: !active
            && encrypted_secrets == 0
            && actionable_approvals == 0
            && malformed_actionable_approvals == 0,
    }
}

fn parse_positive_key_version(name: &str, value: &str) -> Result<i32, String> {
    let version: i32 = value
        .parse()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if version <= 0 {
        return Err(format!("{name} must be a positive integer"));
    }
    Ok(version)
}

fn parse_previous_secret_keys(value: &str) -> Result<Vec<(i32, [u8; 32])>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (version, key) = entry.split_once(':').ok_or_else(|| {
                "ORTYO_SECRETS_PREVIOUS_KEYS must contain version:64hex entries".to_owned()
            })?;
            let version =
                parse_positive_key_version("ORTYO_SECRETS_PREVIOUS_KEYS version", version)?;
            let key = decode_master_key(key).map_err(|_| {
                "ORTYO_SECRETS_PREVIOUS_KEYS keys must be exactly 64 hexadecimal characters"
                    .to_owned()
            })?;
            Ok((version, key))
        })
        .collect()
}

fn keyring_error(error: VersionedKeyringError) -> String {
    match error {
        VersionedKeyringError::InvalidVersion(_) => {
            "secret key versions must be positive integers".to_owned()
        }
        VersionedKeyringError::DuplicateVersion(version) => {
            format!("duplicate secret key version: {version}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KeyMaintenanceConfig;

    #[test]
    fn maintenance_config_requires_only_storage_and_key_material() {
        let key = "22".repeat(32);
        let previous = format!("1:{}", "11".repeat(32));
        let config = KeyMaintenanceConfig::from_lookup(|name| match name {
            "ORTYO_SECRETS_KEY" => Some(key.clone()),
            "ORTYO_SECRETS_KEY_VERSION" => Some("2".to_owned()),
            "ORTYO_SECRETS_PREVIOUS_KEYS" => Some(previous.clone()),
            "ORTYO_HOSTED_DB_PATH" => Some("/tmp/ortyo-key-maintenance.db".to_owned()),
            _ => None,
        })
        .unwrap();

        assert_eq!(config.keyring.active_version(), 2);
        assert_eq!(config.keyring.key(1), Some(&[0x11; 32]));
        assert_eq!(config.db_path, "/tmp/ortyo-key-maintenance.db");
        assert!(config.database_url.is_none());
    }
}
