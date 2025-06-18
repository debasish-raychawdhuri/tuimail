use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),
    
    #[error("Failed to create config directory")]
    CreateDirError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImapSecurity {
    None,
    StartTLS,
    SSL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SmtpSecurity {
    None,
    StartTLS,
    SSL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccount {
    pub name: String,
    pub email: String,
    pub imap_server: String,
    pub imap_port: u16,
    pub imap_security: ImapSecurity,
    pub imap_username: String,
    // Password removed from config - now stored securely
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_security: SmtpSecurity,
    pub smtp_username: String,
    // Password removed from config - now stored securely
    pub signature: Option<String>,
}

impl EmailAccount {
    /// Get IMAP password from secure storage
    pub fn get_imap_password(&self, credentials: &crate::credentials::SecureCredentials) -> Result<String> {
        let account_id = &self.email;
        credentials
            .get_password(account_id, "imap")?
            .ok_or_else(|| anyhow::anyhow!("IMAP password not found for {}", account_id))
    }

    /// Get SMTP password from secure storage
    pub fn get_smtp_password(&self, credentials: &crate::credentials::SecureCredentials) -> Result<String> {
        let account_id = &self.email;
        credentials
            .get_password(account_id, "smtp")?
            .ok_or_else(|| anyhow::anyhow!("SMTP password not found for {}", account_id))
    }

    /// Store IMAP password securely
    pub fn store_imap_password(&self, credentials: &crate::credentials::SecureCredentials, password: &str) -> Result<()> {
        credentials.store_password(&self.email, "imap", password)
    }

    /// Store SMTP password securely
    pub fn store_smtp_password(&self, credentials: &crate::credentials::SecureCredentials, password: &str) -> Result<()> {
        credentials.store_password(&self.email, "smtp", password)
    }
}

impl Default for EmailAccount {
    fn default() -> Self {
        Self {
            name: "Default Account".to_string(),
            email: "user@example.com".to_string(),
            imap_server: "imap.example.com".to_string(),
            imap_port: 993,
            imap_security: ImapSecurity::SSL,
            imap_username: "user@example.com".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_security: SmtpSecurity::StartTLS,
            smtp_username: "user@example.com".to_string(),
            signature: Some("Sent from Email Client".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: String,
    pub show_headers: bool,
    pub refresh_interval: u64,
    pub preview_pane: bool,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            show_headers: false,
            refresh_interval: 300,
            preview_pane: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub accounts: Vec<EmailAccount>,
    pub default_account: usize,
    pub ui: UIConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            accounts: vec![],
            default_account: 0,
            ui: UIConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let path = Path::new(path);
        
        // If the file doesn't exist, return default config
        if !path.exists() {
            return Ok(Config::default());
        }
        
        let content = fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        
        Ok(config)
    }
    
    pub fn save(&self, path: &str) -> Result<(), ConfigError> {
        let path = Path::new(path);
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| ConfigError::CreateDirError)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        
        Ok(())
    }
    
    pub fn get_current_account_safe(&self) -> EmailAccount {
        if self.accounts.is_empty() {
            // Return a default account if none exist (this shouldn't happen in normal usage)
            EmailAccount::default()
        } else if self.default_account >= self.accounts.len() {
            // Return the first account if default index is invalid
            self.accounts[0].clone()
        } else {
            self.accounts[self.default_account].clone()
        }
    }
}
