use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionedKeyringError {
    InvalidVersion(i32),
    DuplicateVersion(i32),
}

#[derive(Clone, PartialEq, Eq)]
pub struct VersionedKeyring {
    active_version: i32,
    keys: BTreeMap<i32, [u8; 32]>,
}

impl std::fmt::Debug for VersionedKeyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VersionedKeyring")
            .field("active_version", &self.active_version)
            .field("versions", &self.keys.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl VersionedKeyring {
    pub fn single(key: [u8; 32]) -> Self {
        Self::new(1, key, []).expect("version 1 is valid")
    }

    pub fn new(
        active_version: i32,
        active_key: [u8; 32],
        previous: impl IntoIterator<Item = (i32, [u8; 32])>,
    ) -> Result<Self, VersionedKeyringError> {
        if active_version <= 0 {
            return Err(VersionedKeyringError::InvalidVersion(active_version));
        }

        let mut keys = BTreeMap::new();
        keys.insert(active_version, active_key);

        for (version, key) in previous {
            if version <= 0 {
                return Err(VersionedKeyringError::InvalidVersion(version));
            }
            if keys.insert(version, key).is_some() {
                return Err(VersionedKeyringError::DuplicateVersion(version));
            }
        }

        Ok(Self {
            active_version,
            keys,
        })
    }

    pub fn active_version(&self) -> i32 {
        self.active_version
    }

    pub fn active_key(&self) -> &[u8; 32] {
        self.keys
            .get(&self.active_version)
            .expect("active key is always present")
    }

    pub fn key(&self, version: i32) -> Option<&[u8; 32]> {
        self.keys.get(&version)
    }

    pub fn entries(&self) -> impl Iterator<Item = (i32, &[u8; 32])> {
        self.keys.iter().map(|(version, key)| (*version, key))
    }

    pub fn versions(&self) -> impl Iterator<Item = i32> + '_ {
        self.keys.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{VersionedKeyring, VersionedKeyringError};

    #[test]
    fn single_key_uses_version_one() {
        let keyring = VersionedKeyring::single([7; 32]);
        assert_eq!(keyring.active_version(), 1);
        assert_eq!(keyring.active_key(), &[7; 32]);
        assert_eq!(keyring.key(1), Some(&[7; 32]));
    }

    #[test]
    fn validates_versions_and_duplicates() {
        assert_eq!(
            VersionedKeyring::new(0, [1; 32], []),
            Err(VersionedKeyringError::InvalidVersion(0))
        );
        assert_eq!(
            VersionedKeyring::new(2, [2; 32], [(0, [1; 32])]),
            Err(VersionedKeyringError::InvalidVersion(0))
        );
        assert_eq!(
            VersionedKeyring::new(2, [2; 32], [(2, [1; 32])]),
            Err(VersionedKeyringError::DuplicateVersion(2))
        );
        assert_eq!(
            VersionedKeyring::new(3, [3; 32], [(1, [1; 32]), (1, [9; 32])]),
            Err(VersionedKeyringError::DuplicateVersion(1))
        );
    }
}
