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
    #[allow(dead_code)]
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
            .join("tuimail")
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
        
        std::fs::write(&file_path, &encrypted)
            .context("Failed to write encrypted password file")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o600))
                .context("Failed to set credential file permissions")?;
        }

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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn delete_password(&self, account_id: &str, password_type: &str) -> Result<()> {
        match self {
            Self::SystemKeyring(manager) => manager.delete_password(account_id, password_type),
            Self::Fallback(manager) => manager.delete_password(account_id, password_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_fallback() -> FallbackCredentialManager {
        let dir = std::env::temp_dir().join("tuimail_cred_test");
        std::fs::create_dir_all(&dir).unwrap();
        FallbackCredentialManager {
            config_dir: dir.to_string_lossy().to_string(),
        }
    }

    #[test]
    fn test_xor_encrypt_decrypt_roundtrip() {
        let mgr = test_fallback();
        let key = mgr.derive_key("test@example.com");
        let plaintext = b"my_secret_password";
        let encrypted = mgr.xor_encrypt(plaintext, &key);
        assert_ne!(&encrypted, plaintext);
        let decrypted = mgr.xor_encrypt(&encrypted, &key);
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let mgr = test_fallback();
        let key1 = mgr.derive_key("user@example.com");
        let key2 = mgr.derive_key("user@example.com");
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_derive_key_different_accounts() {
        let mgr = test_fallback();
        let key1 = mgr.derive_key("alice@example.com");
        let key2 = mgr.derive_key("bob@example.com");
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_store_and_retrieve_password() {
        let mgr = test_fallback();
        mgr.store_password("testuser@cred.com", "imap", "hunter2").unwrap();
        let pass = mgr.get_password("testuser@cred.com", "imap").unwrap();
        assert_eq!(pass, Some("hunter2".to_string()));
        // Cleanup
        mgr.delete_password("testuser@cred.com", "imap").unwrap();
    }

    #[test]
    fn test_get_nonexistent_password() {
        let mgr = test_fallback();
        let pass = mgr.get_password("nobody@cred.com", "smtp").unwrap();
        assert_eq!(pass, None);
    }

    #[test]
    fn test_delete_password() {
        let mgr = test_fallback();
        mgr.store_password("deltest@cred.com", "imap", "pw").unwrap();
        mgr.delete_password("deltest@cred.com", "imap").unwrap();
        let pass = mgr.get_password("deltest@cred.com", "imap").unwrap();
        assert_eq!(pass, None);
    }

    #[test]
    fn test_delete_nonexistent_password() {
        let mgr = test_fallback();
        // Should not error
        mgr.delete_password("ghost@cred.com", "imap").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_credential_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let mgr = test_fallback();
        mgr.store_password("permtest@cred.com", "imap", "secret").unwrap();
        let path = format!("{}/permtest@cred.com_imap.enc", mgr.config_dir);
        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        mgr.delete_password("permtest@cred.com", "imap").unwrap();
    }
}
