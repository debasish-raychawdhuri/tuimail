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
    pub imap_password: String,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_security: SmtpSecurity,
    pub smtp_username: String,
    pub smtp_password: String,
    pub signature: Option<String>,
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
            imap_password: "".to_string(),
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_security: SmtpSecurity::StartTLS,
            smtp_username: "user@example.com".to_string(),
            smtp_password: "".to_string(),
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
    
    pub fn get_current_account(&self) -> Result<&EmailAccount, &'static str> {
        if self.accounts.is_empty() {
            return Err("No accounts configured");
        }
        
        if self.default_account >= self.accounts.len() {
            return Err("Default account index out of bounds");
        }
        
        Ok(&self.accounts[self.default_account])
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
    
    pub fn add_account(&mut self, account: EmailAccount) {
        self.accounts.push(account);
    }
    
    pub fn remove_account(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.accounts.len() {
            return Err("Account index out of bounds");
        }
        
        if self.accounts.len() == 1 {
            return Err("Cannot remove the only account");
        }
        
        self.accounts.remove(index);
        
        // Adjust default account index if needed
        if self.default_account >= self.accounts.len() {
            self.default_account = self.accounts.len() - 1;
        }
        
        Ok(())
    }
    
    pub fn set_default_account(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.accounts.len() {
            return Err("Account index out of bounds");
        }
        
        self.default_account = index;
        Ok(())
    }
}
