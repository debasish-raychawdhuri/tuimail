use anyhow::{Context, Result};
use keyring::{Entry, Error as KeyringError};

/// Secure credential manager using system keyring
#[derive(Clone)]
pub struct CredentialManager {
    app_name: String,
}

impl CredentialManager {
    /// Create a new credential manager
    pub fn new() -> Result<Self> {
        Ok(Self {
            app_name: "email-client".to_string(),
        })
    }

    /// Store a password securely in the system keyring
    pub fn store_password(&self, account_id: &str, password_type: &str, password: &str) -> Result<()> {
        let service = format!("{}-{}", self.app_name, password_type);
        let entry = Entry::new(&service, account_id)
            .context("Failed to create keyring entry")?;
        
        entry.set_password(password)
            .context("Failed to store password in keyring")?;

        // Don't print to stdout during TUI operation - use debug logging instead
        log::debug!("Password stored securely for {} ({})", account_id, password_type);
        Ok(())
    }

    /// Retrieve a password from the system keyring
    pub fn get_password(&self, account_id: &str, password_type: &str) -> Result<Option<String>> {
        let service = format!("{}-{}", self.app_name, password_type);
        let entry = Entry::new(&service, account_id)
            .context("Failed to create keyring entry")?;
        
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to retrieve password: {}", e)),
        }
    }

    /// Delete a password from the system keyring
    pub fn delete_password(&self, account_id: &str, password_type: &str) -> Result<()> {
        let service = format!("{}-{}", self.app_name, password_type);
        let entry = Entry::new(&service, account_id)
            .context("Failed to create keyring entry")?;
        
        match entry.delete_password() {
            Ok(()) => {
                log::debug!("Password deleted for {} ({})", account_id, password_type);
                Ok(())
            }
            Err(KeyringError::NoEntry) => {
                // Password doesn't exist, that's fine
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("Failed to delete password: {}", e)),
        }
    }

    /// Check if the system keyring is available
    pub fn is_available() -> bool {
        // Try to create a test entry to see if keyring is available
        if let Ok(entry) = Entry::new("email-client-test", "test") {
            // Try to set and delete a test password
            if entry.set_password("test").is_ok() {
                let _ = entry.delete_password(); // Clean up
                return true;
            }
        }
        false
    }
}

/// Fallback credential storage for systems without keyring support
#[derive(Clone)]
pub struct FallbackCredentialManager {
    config_dir: String,
}

impl FallbackCredentialManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?
            .join("email_client")
            .join("credentials");
        
        std::fs::create_dir_all(&config_dir)
            .context("Failed to create credentials directory")?;

        Ok(Self {
            config_dir: config_dir.to_string_lossy().to_string(),
        })
    }

    pub fn store_password(&self, account_id: &str, password_type: &str, password: &str) -> Result<()> {
        // For fallback, we'll use a simple encrypted file
        // This is less secure than system keyring but better than plain text
        let file_path = format!("{}/{}_{}.enc", self.config_dir, account_id, password_type);
        
        // Simple XOR encryption with a key derived from username
        let key = self.derive_key(account_id);
        let encrypted = self.xor_encrypt(password.as_bytes(), &key);
        
        std::fs::write(&file_path, encrypted)
            .context("Failed to write encrypted password file")?;

        log::warn!("Password stored with fallback encryption for {} ({})", account_id, password_type);
        log::warn!("Note: For better security, install GNOME Keyring or similar");
        Ok(())
    }

    pub fn get_password(&self, account_id: &str, password_type: &str) -> Result<Option<String>> {
        let file_path = format!("{}/{}_{}.enc", self.config_dir, account_id, password_type);
        
        if !std::path::Path::new(&file_path).exists() {
            return Ok(None);
        }

        let encrypted = std::fs::read(&file_path)
            .context("Failed to read encrypted password file")?;

        let key = self.derive_key(account_id);
        let decrypted = self.xor_encrypt(&encrypted, &key);
        
        let password = String::from_utf8(decrypted)
            .context("Failed to decode password")?;

        Ok(Some(password))
    }

    pub fn delete_password(&self, account_id: &str, password_type: &str) -> Result<()> {
        let file_path = format!("{}/{}_{}.enc", self.config_dir, account_id, password_type);
        
        if std::path::Path::new(&file_path).exists() {
            std::fs::remove_file(&file_path)
                .context("Failed to delete password file")?;
        }

        log::debug!("Password deleted for {} ({})", account_id, password_type);
        Ok(())
    }

    fn derive_key(&self, account_id: &str) -> Vec<u8> {
        // Simple key derivation - in production, use proper KDF like PBKDF2
        let mut key = Vec::new();
        let account_bytes = account_id.as_bytes();
        for i in 0..32 {
            key.push(account_bytes[i % account_bytes.len()] ^ (i as u8));
        }
        key
    }

    fn xor_encrypt(&self, data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ key[i % key.len()])
            .collect()
    }
}

/// Unified credential manager that tries system keyring first, then falls back
#[derive(Clone)]
pub enum SecureCredentials {
    SystemKeyring(CredentialManager),
    Fallback(FallbackCredentialManager),
}

impl SecureCredentials {
    /// Create a new secure credential manager
    pub fn new() -> Result<Self> {
        if CredentialManager::is_available() {
            Ok(Self::SystemKeyring(CredentialManager::new()?))
        } else {
            Ok(Self::Fallback(FallbackCredentialManager::new()?))
        }
    }

    pub fn store_password(&self, account_id: &str, password_type: &str, password: &str) -> Result<()> {
        match self {
            Self::SystemKeyring(manager) => manager.store_password(account_id, password_type, password),
            Self::Fallback(manager) => manager.store_password(account_id, password_type, password),
        }
    }

    pub fn get_password(&self, account_id: &str, password_type: &str) -> Result<Option<String>> {
        match self {
            Self::SystemKeyring(manager) => manager.get_password(account_id, password_type),
            Self::Fallback(manager) => manager.get_password(account_id, password_type),
        }
    }

    pub fn delete_password(&self, account_id: &str, password_type: &str) -> Result<()> {
        match self {
            Self::SystemKeyring(manager) => manager.delete_password(account_id, password_type),
            Self::Fallback(manager) => manager.delete_password(account_id, password_type),
        }
    }
}
