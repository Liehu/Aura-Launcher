//! Credential storage (review 54 §7/§8): the auth layer depends only on
//! the `CredentialStore` trait; the OS implementation (Windows Credential
//! Manager) never leaks secrets above the transport boundary. Config.toml
//! NEVER carries credentials — only the store does.

use thiserror::Error;

use super::types::{Credential, CredentialKey};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("credential store io failed: {0}")]
    Io(String),
    #[error("credential store unavailable on this platform")]
    Unavailable,
}

pub trait CredentialStore: Send + Sync {
    fn load(&self, key: &CredentialKey) -> Result<Option<Credential>, StoreError>;
    fn save(&mut self, key: &CredentialKey, credential: &Credential) -> Result<(), StoreError>;
    fn delete(&mut self, key: &CredentialKey) -> Result<(), StoreError>;
}

/// In-memory store: default and test backend. Process death clears
/// credentials (memory-only by design for ephemeral sessions). Debug is
/// MANUALLY implemented to keep the entry secrets out of any formatted
/// output (review 54 SS40).
#[derive(Default)]
pub struct InMemoryCredentialStore {
    entries: std::collections::HashMap<String, Credential>,
}

impl std::fmt::Debug for InMemoryCredentialStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryCredentialStore")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn load(&self, key: &CredentialKey) -> Result<Option<Credential>, StoreError> {
        Ok(self.entries.get(&key.storage_key()).cloned())
    }
    fn save(&mut self, key: &CredentialKey, credential: &Credential) -> Result<(), StoreError> {
        self.entries.insert(key.storage_key(), credential.clone());
        Ok(())
    }
    fn delete(&mut self, key: &CredentialKey) -> Result<(), StoreError> {
        self.entries.remove(&key.storage_key());
        Ok(())
    }
}

/// Windows Credential Manager backend (review 54 §7, Windows-first). The
/// credential JSON is stored as a generic credential named by the
/// canonical `CredentialKey::storage_key()`.
#[derive(Default)]
pub struct WindowsCredentialStore;

impl WindowsCredentialStore {
    pub fn new() -> Self {
        Self
    }
    fn key_str(key: &CredentialKey) -> Vec<u16> {
        key.storage_key().encode_utf16().chain(std::iter::once(0)).collect()
    }
}

impl CredentialStore for WindowsCredentialStore {
    fn load(&self, key: &CredentialKey) -> Result<Option<Credential>, StoreError> {
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};
        let target = Self::key_str(key);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // not-found is NOT an error for `load`: it simply means no
        // credential exists yet
        if unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0, &mut cred) }.is_err()
        {
            return Ok(None);
        }
        unsafe {
            use windows::Win32::Security::Credentials::CredFree;
            let blob = *cred;
            let bytes = std::slice::from_raw_parts(blob.CredentialBlob, blob.CredentialBlobSize as usize);
            let parsed: Result<Credential, _> = serde_json::from_slice(bytes);
            CredFree(cred as *const _ as *mut _);
            parsed
                .map(Some)
                .map_err(|e| StoreError::Io(format!("stored credential parse: {e}")))
        }
    }
    fn save(&mut self, key: &CredentialKey, credential: &Credential) -> Result<(), StoreError> {
        use windows::core::PWSTR;
        use windows::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };
        let json = serde_json::to_vec(credential)
            .map_err(|e| StoreError::Io(format!("credential serialize: {e}")))?;
        let mut target = Self::key_str(key);
        let mut cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlob: json.as_ptr() as *mut u8,
            CredentialBlobSize: json.len() as u32,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe {
            CredWriteW(&mut cred, 0).map_err(|e| StoreError::Io(e.to_string()))
        }
    }
    fn delete(&mut self, key: &CredentialKey) -> Result<(), StoreError> {
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};
        let target = Self::key_str(key);
        unsafe {
            CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0)
                .map_err(|e| StoreError::Io(e.to_string()))
        }
    }
}
