use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use zeroize::Zeroize;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use crate::scanner::ScannerError;
use crate::storage;

// ─── Data Structures ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMeta {
    pub salt: String,           // Argon2 salt (base64)
    pub nonce: String,          // Nonce for encrypted index (base64)
    pub encrypted_index: String, // AES-256-GCM encrypted index (base64)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultIndex {
    pub documents: HashMap<String, VaultDocEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDocEntry {
    pub id: String,
    pub name: String,
    pub date_added: String,
    pub original_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDocDto {
    pub id: String,
    pub name: String,
    pub date_added: String,
}

// ─── Vault Manager ───────────────────────────────────────────────

pub struct VaultManager {
    vault_dir: PathBuf,
    derived_key: Option<[u8; 32]>,
}

impl VaultManager {
    pub fn new() -> Self {
        let vault_dir = storage::config_dir_pub().join("vault");
        let _ = fs::create_dir_all(&vault_dir);
        let _ = fs::create_dir_all(vault_dir.join("docs"));
        Self {
            vault_dir,
            derived_key: None,
        }
    }

    fn meta_path(&self) -> PathBuf {
        self.vault_dir.join("vault.meta")
    }

    fn doc_path(&self, id: &str) -> PathBuf {
        self.vault_dir.join("docs").join(format!("{}.enc", id))
    }

    pub fn is_setup(&self) -> bool {
        self.meta_path().exists()
    }

    pub fn is_unlocked(&self) -> bool {
        self.derived_key.is_some()
    }

    /// Derive a 256-bit key from password using Argon2id
    fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], ScannerError> {
        let argon2 = Argon2::default();
        let mut output_key = [0u8; 32];

        argon2
            .hash_password_into(password.as_bytes(), salt, &mut output_key)
            .map_err(|e| ScannerError::SystemError(format!("Key derivation failed: {}", e)))?;

        Ok(output_key)
    }

    /// Set master password for the first time
    pub fn set_password(&mut self, password: &str) -> Result<(), ScannerError> {
        if self.is_setup() {
            return Err(ScannerError::SystemError("Vault already setup".into()));
        }

        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);

        let key = Self::derive_key(password, &salt)?;

        // Encrypt empty index
        let index = VaultIndex::default();
        let index_json = serde_json::to_vec(&index)
            .map_err(|e| ScannerError::SystemError(format!("Serialize index: {}", e)))?;

        let (encrypted, nonce) = self.encrypt_data(&key, &index_json)?;

        let meta = VaultMeta {
            salt: BASE64.encode(&salt),
            nonce: BASE64.encode(&nonce),
            encrypted_index: BASE64.encode(&encrypted),
        };

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| ScannerError::SystemError(format!("Serialize meta: {}", e)))?;
        fs::write(self.meta_path(), meta_json)
            .map_err(|e| ScannerError::SystemError(format!("Write vault meta: {}", e)))?;

        self.derived_key = Some(key);
        Ok(())
    }

    /// Unlock the vault with master password
    pub fn unlock(&mut self, password: &str) -> Result<(), ScannerError> {
        let meta = self.load_meta()?;
        let salt = BASE64
            .decode(&meta.salt)
            .map_err(|e| ScannerError::SystemError(format!("Decode salt: {}", e)))?;

        let key = Self::derive_key(password, &salt)?;

        // Verify by trying to decrypt the index
        let _ = self.decrypt_index(&key, &meta)?;

        self.derived_key = Some(key);
        Ok(())
    }

    /// Lock the vault (clear key from memory)
    pub fn lock(&mut self) {
        if let Some(ref mut key) = self.derived_key {
            key.zeroize();
        }
        self.derived_key = None;
    }

    /// Add a document to the vault
    pub fn add_document(
        &self,
        id: &str,
        name: &str,
        png_data: &[u8],
    ) -> Result<(), ScannerError> {
        let key = self.require_key()?;

        let (encrypted, nonce) = self.encrypt_data(&key, png_data)?;

        // Write encrypted document: nonce (12 bytes) || ciphertext
        let mut file_data = Vec::with_capacity(12 + encrypted.len());
        file_data.extend_from_slice(&nonce);
        file_data.extend_from_slice(&encrypted);
        fs::write(self.doc_path(id), file_data)
            .map_err(|e| ScannerError::SystemError(format!("Write vault doc: {}", e)))?;

        // Update index
        let mut index = self.load_index()?;
        index.documents.insert(
            id.to_string(),
            VaultDocEntry {
                id: id.to_string(),
                name: name.to_string(),
                date_added: chrono::Local::now().format("%d/%m/%Y %H:%M").to_string(),
                original_size: png_data.len() as u64,
            },
        );
        self.save_index(&index)?;

        Ok(())
    }

    /// List documents in the vault
    pub fn list_documents(&self) -> Result<Vec<VaultDocDto>, ScannerError> {
        let index = self.load_index()?;
        Ok(index
            .documents
            .values()
            .map(|e| VaultDocDto {
                id: e.id.clone(),
                name: e.name.clone(),
                date_added: e.date_added.clone(),
            })
            .collect())
    }

    /// Open (decrypt) a document from the vault
    pub fn open_document(&self, id: &str) -> Result<Vec<u8>, ScannerError> {
        let key = self.require_key()?;

        let file_data = fs::read(self.doc_path(id))
            .map_err(|e| ScannerError::SystemError(format!("Read vault doc: {}", e)))?;

        if file_data.len() < 12 {
            return Err(ScannerError::SystemError("Corrupt vault document".into()));
        }

        let nonce = &file_data[..12];
        let ciphertext = &file_data[12..];

        self.decrypt_data(&key, nonce, ciphertext)
    }

    /// Remove a document from the vault
    pub fn remove_document(&self, id: &str) -> Result<(), ScannerError> {
        let _ = fs::remove_file(self.doc_path(id));

        let mut index = self.load_index()?;
        index.documents.remove(id);
        self.save_index(&index)?;

        Ok(())
    }

    // ─── Private helpers ─────────────────────────────────────────

    fn require_key(&self) -> Result<[u8; 32], ScannerError> {
        self.derived_key
            .ok_or_else(|| ScannerError::SystemError("Vault is locked".into()))
    }

    fn load_meta(&self) -> Result<VaultMeta, ScannerError> {
        let json = fs::read_to_string(self.meta_path())
            .map_err(|e| ScannerError::SystemError(format!("Read vault meta: {}", e)))?;
        serde_json::from_str(&json)
            .map_err(|e| ScannerError::SystemError(format!("Parse vault meta: {}", e)))
    }

    fn encrypt_data(&self, key: &[u8; 32], data: &[u8]) -> Result<(Vec<u8>, [u8; 12]), ScannerError> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| ScannerError::SystemError(format!("AES-GCM init: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted = cipher
            .encrypt(nonce, data)
            .map_err(|e| ScannerError::SystemError(format!("Encryption failed: {}", e)))?;

        Ok((encrypted, nonce_bytes))
    }

    fn decrypt_data(&self, key: &[u8; 32], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, ScannerError> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| ScannerError::SystemError(format!("AES-GCM init: {}", e)))?;

        let nonce = Nonce::from_slice(nonce);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| ScannerError::SystemError("Wrong password or corrupt data".into()))
    }

    fn decrypt_index(&self, key: &[u8; 32], meta: &VaultMeta) -> Result<VaultIndex, ScannerError> {
        let nonce = BASE64
            .decode(&meta.nonce)
            .map_err(|e| ScannerError::SystemError(format!("Decode nonce: {}", e)))?;
        let encrypted = BASE64
            .decode(&meta.encrypted_index)
            .map_err(|e| ScannerError::SystemError(format!("Decode index: {}", e)))?;

        let decrypted = self.decrypt_data(key, &nonce, &encrypted)?;

        serde_json::from_slice(&decrypted)
            .map_err(|e| ScannerError::SystemError(format!("Parse index: {}", e)))
    }

    fn load_index(&self) -> Result<VaultIndex, ScannerError> {
        let key = self.require_key()?;
        let meta = self.load_meta()?;
        self.decrypt_index(&key, &meta)
    }

    fn save_index(&self, index: &VaultIndex) -> Result<(), ScannerError> {
        let key = self.require_key()?;

        let index_json = serde_json::to_vec(index)
            .map_err(|e| ScannerError::SystemError(format!("Serialize index: {}", e)))?;

        let (encrypted, nonce) = self.encrypt_data(&key, &index_json)?;

        let mut meta = self.load_meta()?;
        meta.nonce = BASE64.encode(&nonce);
        meta.encrypted_index = BASE64.encode(&encrypted);

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| ScannerError::SystemError(format!("Serialize meta: {}", e)))?;
        fs::write(self.meta_path(), meta_json)
            .map_err(|e| ScannerError::SystemError(format!("Write vault meta: {}", e)))?;

        Ok(())
    }
}

impl Drop for VaultManager {
    fn drop(&mut self) {
        self.lock();
    }
}
