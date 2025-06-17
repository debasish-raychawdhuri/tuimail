use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use thiserror::Error;

use crate::config::Config;
use crate::credentials::SecureCredentials;
use crate::email::{Email, EmailClient};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Email error: {0}")]
    EmailError(#[from] crate::email::EmailError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeField {
    To,
    Subject,
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Compose,
    ViewEmail,
    FolderList,
    AccountSettings,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    AccountList,
    FolderList,
    EmailList,
    EmailView,
    ComposeForm,
}

/// Represents a folder item in the hierarchical view
#[derive(Debug, Clone)]
pub enum FolderItem {
    Account {
        name: String,
        email: String,
        index: usize,
        expanded: bool,
    },
    Folder {
        name: String,
        account_index: usize,
        full_path: String, // For IMAP folder path
    },
}

/// Account-specific folder and email data
pub struct AccountData {
    pub folders: Vec<String>,
    pub emails: Vec<Email>,
    pub selected_folder_idx: usize,
    pub email_client: Option<EmailClient>,
}

impl AccountData {
    pub fn new() -> Self {
        Self {
            folders: vec!["INBOX".to_string()],
            emails: Vec::new(),
            selected_folder_idx: 0,
            email_client: None,
        }
    }
}

pub struct App {
    pub config: Config,
    pub credentials: SecureCredentials,
    pub should_quit: bool,
    pub mode: AppMode,
    pub focus: FocusPanel,
    
    // Multi-account support
    pub accounts: std::collections::HashMap<usize, AccountData>,
    pub current_account_idx: usize,
    pub folder_items: Vec<FolderItem>, // Hierarchical folder view
    pub selected_folder_item_idx: usize,
    
    // Current view state (for the selected account/folder)
    pub emails: Vec<Email>,
    pub selected_email_idx: Option<usize>,
    
    pub compose_email: Email,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub message_timeout: Option<Instant>,
    
    // Scrolling state
    pub email_scroll_offset: usize,
    pub folder_scroll_offset: usize,
    pub email_view_scroll: usize,
    
    // Sync status
    pub last_sync: Option<DateTime<Local>>,
    pub is_syncing: bool,
    
    // Compose form state
    pub compose_field: ComposeField,
}

impl App {
    pub fn new(config: Config) -> Self {
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] App::new() called with {} accounts", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), config.accounts.len());
            }
        }
        
        let credentials = SecureCredentials::new()
            .expect("Failed to initialize secure credential storage");
        
        // Initialize accounts data structure
        let mut accounts = std::collections::HashMap::new();
        let mut folder_items = Vec::new();
        
        // Create folder items for each account
        for (index, account) in config.accounts.iter().enumerate() {
            accounts.insert(index, AccountData::new());
            
            folder_items.push(FolderItem::Account {
                name: account.name.clone(),
                email: account.email.clone(),
                index,
                expanded: index == config.default_account, // Expand default account
            });
            
            // Add default folders for expanded accounts
            if index == config.default_account {
                folder_items.push(FolderItem::Folder {
                    name: "INBOX".to_string(),
                    account_index: index,
                    full_path: "INBOX".to_string(),
                });
            }
        }
        
        let current_account_idx = config.default_account;
        
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] App::new() completed, default account: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), current_account_idx);
            }
        }
            
        Self {
            config,
            credentials,
            should_quit: false,
            mode: AppMode::Normal,
            focus: FocusPanel::FolderList,
            
            // Multi-account support
            accounts,
            current_account_idx,
            folder_items,
            selected_folder_item_idx: 0,
            
            // Current view state
            emails: Vec::new(),
            selected_email_idx: None,
            
            compose_email: Email::new(),
            error_message: None,
            info_message: None,
            message_timeout: None,
            
            email_scroll_offset: 0,
            folder_scroll_offset: 0,
            email_view_scroll: 0,
            last_sync: None,
            is_syncing: false,
            compose_field: ComposeField::To,
        }
    }
    
    // Multi-account support methods
    
    /// Get the current account data
    pub fn current_account(&self) -> Option<&AccountData> {
        self.accounts.get(&self.current_account_idx)
    }
    
    /// Get mutable reference to current account data
    pub fn current_account_mut(&mut self) -> Option<&mut AccountData> {
        self.accounts.get_mut(&self.current_account_idx)
    }
    
    /// Switch to a different account
    pub fn switch_account(&mut self, account_idx: usize) -> AppResult<()> {
        if account_idx < self.config.accounts.len() {
            self.current_account_idx = account_idx;
            self.rebuild_folder_items();
            self.init_account(account_idx)?;
            Ok(())
        } else {
            Err(AppError::EmailError(crate::email::EmailError::ImapError(
                "Invalid account index".to_string()
            )))
        }
    }
    
    /// Toggle account expansion in folder view
    pub fn toggle_account_expansion(&mut self, account_idx: usize) {
        // Find and toggle the account
        for item in &mut self.folder_items {
            if let FolderItem::Account { index, expanded, .. } = item {
                if *index == account_idx {
                    *expanded = !*expanded;
                    break;
                }
            }
        }
        self.rebuild_folder_items();
    }
    
    /// Rebuild the folder items list based on account expansion states
    pub fn rebuild_folder_items(&mut self) {
        let mut new_items = Vec::new();
        
        for (account_idx, account_config) in self.config.accounts.iter().enumerate() {
            // Find if this account is expanded
            let expanded = self.folder_items.iter()
                .find_map(|item| {
                    if let FolderItem::Account { index, expanded, .. } = item {
                        if *index == account_idx {
                            Some(*expanded)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or(account_idx == self.current_account_idx);
            
            new_items.push(FolderItem::Account {
                name: account_config.name.clone(),
                email: account_config.email.clone(),
                index: account_idx,
                expanded,
            });
            
            // Add folders if expanded
            if expanded {
                if let Some(account_data) = self.accounts.get(&account_idx) {
                    for folder in &account_data.folders {
                        new_items.push(FolderItem::Folder {
                            name: folder.clone(),
                            account_index: account_idx,
                            full_path: folder.clone(),
                        });
                    }
                }
            }
        }
        
        self.folder_items = new_items;
        
        // Ensure selected index is valid
        if self.selected_folder_item_idx >= self.folder_items.len() {
            self.selected_folder_item_idx = self.folder_items.len().saturating_sub(1);
        }
    }
    
    /// Initialize a specific account (create email client, load folders)
    pub fn init_account(&mut self, account_idx: usize) -> AppResult<()> {
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] Initializing account index: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), account_idx);
            }
        }
        
        if account_idx >= self.config.accounts.len() {
            return Err(AppError::EmailError(crate::email::EmailError::ImapError(
                format!("Invalid account index: {} >= {}", account_idx, self.config.accounts.len())
            )));
        }
        
        let account_config = &self.config.accounts[account_idx];
        
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] Creating EmailClient for: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), account_config.email);
            }
        }
        
        // Create email client for this account
        let client = EmailClient::new(
            account_config.clone(),
            self.credentials.clone(),
        );
        
        // Get or create account data
        let account_data = self.accounts.entry(account_idx).or_insert_with(AccountData::new);
        account_data.email_client = Some(client);
        
        // Load folders for this account
        self.load_folders_for_account(account_idx)?;
        
        Ok(())
    }
    
    /// Load folders for a specific account
    pub fn load_folders_for_account(&mut self, account_idx: usize) -> AppResult<()> {
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] Loading folders for account: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), account_idx);
            }
        }
        
        if let Some(account_data) = self.accounts.get_mut(&account_idx) {
            if let Some(client) = &account_data.email_client {
                match client.list_folders() {
                    Ok(folders) => {
                        // Debug logging
                        if std::env::var("EMAIL_DEBUG").is_ok() {
                            let log_file = "/tmp/email_client_debug.log";
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .write(true)
                                .append(true)
                                .open(log_file) 
                            {
                                use std::io::Write;
                                let _ = writeln!(file, "[{}] Found {} folders for account {}", 
                                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), folders.len(), account_idx);
                            }
                        }
                        
                        account_data.folders = folders;
                        self.rebuild_folder_items();
                        Ok(())
                    }
                    Err(e) => {
                        // Debug logging
                        if std::env::var("EMAIL_DEBUG").is_ok() {
                            let log_file = "/tmp/email_client_debug.log";
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .write(true)
                                .append(true)
                                .open(log_file) 
                            {
                                use std::io::Write;
                                let _ = writeln!(file, "[{}] Error loading folders for account {}: {}", 
                                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), account_idx, e);
                            }
                        }
                        
                        self.show_error(&format!("Failed to load folders for account {}: {}", account_idx, e));
                        Err(AppError::EmailError(e))
                    }
                }
            } else {
                Err(AppError::EmailError(crate::email::EmailError::ImapError(
                    "No email client for account".to_string()
                )))
            }
        } else {
            Err(AppError::EmailError(crate::email::EmailError::ImapError(
                "Account not found".to_string()
            )))
        }
    }
    
    /// Get currently selected folder info
    pub fn get_selected_folder_info(&self) -> Option<(usize, String)> {
        if let Some(item) = self.folder_items.get(self.selected_folder_item_idx) {
            match item {
                FolderItem::Folder { account_index, full_path, .. } => {
                    Some((*account_index, full_path.clone()))
                }
                _ => None,
            }
        } else {
            None
        }
    }
    
    /// Load emails for the currently selected folder
    pub fn load_emails_for_selected_folder(&mut self) -> AppResult<()> {
        if let Some((account_idx, folder_path)) = self.get_selected_folder_info() {
            self.load_emails_for_account_folder(account_idx, &folder_path)
        } else {
            Ok(()) // No folder selected
        }
    }
    
    /// Load emails for a specific account and folder
    pub fn load_emails_for_account_folder(&mut self, account_idx: usize, folder: &str) -> AppResult<()> {
        if let Some(account_data) = self.accounts.get_mut(&account_idx) {
            if let Some(client) = &account_data.email_client {
                match client.fetch_emails(folder, 50) {
                    Ok(emails) => {
                        account_data.emails = emails;
                        
                        // Update legacy fields for backward compatibility
                        if account_idx == self.current_account_idx {
                            self.emails = account_data.emails.clone();
                        }
                        
                        Ok(())
                    }
                    Err(e) => {
                        self.show_error(&format!("Failed to load emails: {}", e));
                        Err(AppError::EmailError(e))
                    }
                }
            } else {
                Err(AppError::EmailError(crate::email::EmailError::ImapError(
                    "No email client for account".to_string()
                )))
            }
        } else {
            Err(AppError::EmailError(crate::email::EmailError::ImapError(
                "Account not found".to_string()
            )))
        }
    }
    
    pub fn init(&mut self) -> AppResult<()> {
        // Debug logging
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] App::init() called", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
            }
        }
        
        // Validate that we have accounts configured
        if self.config.accounts.is_empty() {
            return Err(AppError::EmailError(
                crate::email::EmailError::ImapError("No email accounts configured".to_string())
            ));
        }
        
        // Initialize the current account only (don't initialize all accounts at startup)
        match self.init_account(self.current_account_idx) {
            Ok(()) => {
                // Debug logging
                if std::env::var("EMAIL_DEBUG").is_ok() {
                    let log_file = "/tmp/email_client_debug.log";
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(log_file) 
                    {
                        use std::io::Write;
                        let _ = writeln!(file, "[{}] Successfully initialized account {}", 
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), self.current_account_idx);
                    }
                }
            }
            Err(e) => {
                // Show error but don't fail completely - allow user to switch accounts
                self.show_error(&format!("Failed to initialize default account: {}", e));
                
                // Debug logging
                if std::env::var("EMAIL_DEBUG").is_ok() {
                    let log_file = "/tmp/email_client_debug.log";
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(log_file) 
                    {
                        use std::io::Write;
                        let _ = writeln!(file, "[{}] Failed to initialize account {}: {}", 
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), self.current_account_idx, e);
                    }
                }
                
                // Continue with default folder structure
                self.rebuild_folder_items();
                return Ok(()); // Don't fail completely
            }
        }
        
        // Load emails for the first folder of the current account
        if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
            if !account_data.folders.is_empty() {
                let folder = account_data.folders[0].clone();
                if let Err(e) = self.load_emails_for_account_folder(self.current_account_idx, &folder) {
                    self.show_error(&format!("Failed to load emails: {}", e));
                }
            }
        }
        
        Ok(())
    }
    
    pub fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::Compose => self.handle_compose_mode(key),
            AppMode::ViewEmail => self.handle_view_mode(key),
            AppMode::FolderList => self.handle_folder_list_mode(key),
            AppMode::AccountSettings => self.handle_settings_mode(key),
            AppMode::Help => self.handle_help_mode(key),
        }
    }
    
    fn handle_normal_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                Ok(())
            }
            KeyCode::Char('c') => {
                self.mode = AppMode::Compose;
                self.focus = FocusPanel::ComposeForm;
                self.compose_email = Email::new();
                self.compose_field = ComposeField::To;
                Ok(())
            }
            KeyCode::Char('r') => {
                // Refresh emails for the currently selected folder
                if let Err(e) = self.load_emails_for_selected_folder() {
                    self.show_error(&format!("Failed to refresh emails: {}", e));
                } else {
                    self.show_info("Emails refreshed");
                }
                Ok(())
            }
            KeyCode::Char('f') => {
                self.mode = AppMode::FolderList;
                self.focus = FocusPanel::FolderList;
                Ok(())
            }
            KeyCode::Char('s') => {
                self.mode = AppMode::AccountSettings;
                Ok(())
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::Help;
                Ok(())
            }
            KeyCode::Up => {
                self.select_prev_email();
                Ok(())
            }
            KeyCode::Down => {
                self.select_next_email();
                Ok(())
            }
            KeyCode::Enter => {
                if let Some(idx) = self.selected_email_idx {
                    if idx < self.emails.len() {
                        self.mode = AppMode::ViewEmail;
                        
                        // Mark as read
                        if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
                            if let Some(client) = &account_data.email_client {
                                let email = &self.emails[idx];
                                if !email.seen {
                                    if let Err(e) = client.mark_as_read(email) {
                                        self.show_error(&format!("Failed to mark email as read: {}", e));
                                    } else {
                                        // Update local state
                                        self.emails[idx].seen = true;
                                    }
                                }
                            }
                        }
                    } else {
                        self.show_error("Invalid email selection");
                    }
                } else {
                    self.show_error("No email selected");
                }
                Ok(())
            }
            KeyCode::Delete => {
                self.delete_selected_email()?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_compose_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.focus = FocusPanel::EmailList;
                self.compose_field = ComposeField::To;
                Ok(())
            }
            KeyCode::Tab => {
                // Move to next field
                self.compose_field = match self.compose_field {
                    ComposeField::To => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::To,
                };
                Ok(())
            }
            KeyCode::BackTab => {
                // Move to previous field
                self.compose_field = match self.compose_field {
                    ComposeField::To => ComposeField::Body,
                    ComposeField::Subject => ComposeField::To,
                    ComposeField::Body => ComposeField::Subject,
                };
                Ok(())
            }
            KeyCode::Up => {
                // Move to previous field
                self.compose_field = match self.compose_field {
                    ComposeField::To => ComposeField::Body,
                    ComposeField::Subject => ComposeField::To,
                    ComposeField::Body => ComposeField::Subject,
                };
                Ok(())
            }
            KeyCode::Down => {
                // Move to next field
                self.compose_field = match self.compose_field {
                    ComposeField::To => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::To,
                };
                Ok(())
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_email()?;
                Ok(())
            }
            KeyCode::Char(c) => {
                // Add character to current field
                match self.compose_field {
                    ComposeField::To => {
                        let to_string = self.compose_email.to.iter()
                            .map(|addr| addr.address.clone())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let new_to = format!("{}{}", to_string, c);
                        
                        // Parse the to field
                        self.compose_email.to.clear();
                        for addr in new_to.split(',') {
                            let addr = addr.trim();
                            if !addr.is_empty() {
                                self.compose_email.to.push(crate::email::EmailAddress {
                                    name: None,
                                    address: addr.to_string(),
                                });
                            }
                        }
                    }
                    ComposeField::Subject => {
                        self.compose_email.subject.push(c);
                    }
                    ComposeField::Body => {
                        if let Some(ref mut body) = self.compose_email.body_text {
                            body.push(c);
                        } else {
                            self.compose_email.body_text = Some(c.to_string());
                        }
                    }
                }
                Ok(())
            }
            KeyCode::Backspace => {
                // Remove character from current field
                match self.compose_field {
                    ComposeField::To => {
                        if let Some(last_addr) = self.compose_email.to.last_mut() {
                            if !last_addr.address.is_empty() {
                                last_addr.address.pop();
                                if last_addr.address.is_empty() {
                                    self.compose_email.to.pop();
                                }
                            }
                        }
                    }
                    ComposeField::Subject => {
                        self.compose_email.subject.pop();
                    }
                    ComposeField::Body => {
                        if let Some(ref mut body) = self.compose_email.body_text {
                            body.pop();
                        }
                    }
                }
                Ok(())
            }
            KeyCode::Enter => {
                // In body field, add newline
                if self.compose_field == ComposeField::Body {
                    if let Some(ref mut body) = self.compose_email.body_text {
                        body.push('\n');
                    } else {
                        self.compose_email.body_text = Some("\n".to_string());
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_view_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.email_view_scroll = 0; // Reset scroll when exiting
                Ok(())
            }
            KeyCode::Up => {
                if self.email_view_scroll > 0 {
                    self.email_view_scroll -= 1;
                }
                Ok(())
            }
            KeyCode::Down => {
                self.email_view_scroll += 1;
                Ok(())
            }
            KeyCode::PageUp => {
                self.email_view_scroll = self.email_view_scroll.saturating_sub(10);
                Ok(())
            }
            KeyCode::PageDown => {
                self.email_view_scroll += 10;
                Ok(())
            }
            KeyCode::Home => {
                self.email_view_scroll = 0;
                Ok(())
            }
            KeyCode::Char('r') => {
                self.reply_to_email()?;
                Ok(())
            }
            KeyCode::Char('f') => {
                self.forward_email()?;
                Ok(())
            }
            KeyCode::Char('d') => {
                self.delete_selected_email()?;
                self.mode = AppMode::Normal;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_folder_list_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.focus = FocusPanel::FolderList;
                Ok(())
            }
            KeyCode::Up => {
                if !self.folder_items.is_empty() && self.selected_folder_item_idx > 0 {
                    self.selected_folder_item_idx -= 1;
                }
                Ok(())
            }
            KeyCode::Down => {
                if !self.folder_items.is_empty() && self.selected_folder_item_idx < self.folder_items.len().saturating_sub(1) {
                    self.selected_folder_item_idx += 1;
                }
                Ok(())
            }
            KeyCode::Enter => {
                if let Some(item) = self.folder_items.get(self.selected_folder_item_idx).cloned() {
                    match item {
                        crate::app::FolderItem::Account { index, .. } => {
                            // Toggle account expansion
                            self.toggle_account_expansion(index);
                        }
                        crate::app::FolderItem::Folder { account_index, full_path, .. } => {
                            // Select folder and switch to normal mode
                            self.current_account_idx = account_index;
                            self.mode = AppMode::Normal;
                            self.focus = FocusPanel::EmailList;
                            
                            // Load emails for the selected folder
                            if let Err(e) = self.load_emails_for_account_folder(account_index, &full_path) {
                                self.show_error(&format!("Failed to load emails: {}", e));
                            }
                        }
                    }
                } else {
                    self.show_error("Invalid selection");
                }
                Ok(())
            }
            KeyCode::Char(' ') => {
                // Space bar also toggles account expansion
                if let Some(item) = self.folder_items.get(self.selected_folder_item_idx).cloned() {
                    if let crate::app::FolderItem::Account { index, .. } = item {
                        self.toggle_account_expansion(index);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_settings_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_help_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = AppMode::Normal;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    pub fn select_next_email(&mut self) {
        if self.emails.is_empty() {
            self.selected_email_idx = None;
            return;
        }
        
        match self.selected_email_idx {
            Some(idx) => {
                if idx < self.emails.len().saturating_sub(1) {
                    self.selected_email_idx = Some(idx + 1);
                }
                // If we're at the last email, stay there
            }
            None => {
                // If nothing is selected, select the first email
                self.selected_email_idx = Some(0);
            }
        }
    }
    
    pub fn select_prev_email(&mut self) {
        if self.emails.is_empty() {
            self.selected_email_idx = None;
            return;
        }
        
        match self.selected_email_idx {
            Some(idx) => {
                if idx > 0 {
                    self.selected_email_idx = Some(idx - 1);
                }
                // If we're at the first email, stay there
            }
            None => {
                // If nothing is selected, select the first email
                self.selected_email_idx = Some(0);
            }
        }
    }
    
    pub fn reply_to_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            let original = &self.emails[idx];
            
            let mut reply = Email::new();
            reply.subject = if original.subject.starts_with("Re: ") {
                original.subject.clone()
            } else {
                format!("Re: {}", original.subject)
            };
            
            // Set recipient to the original sender
            reply.to = original.from.clone();
            
            // Set body with quoted original
            if let Some(body) = &original.body_text {
                reply.body_text = Some(format!(
                    "\n\nOn {} wrote:\n{}", 
                    original.date.format("%Y-%m-%d %H:%M"),
                    body.lines().map(|line| format!("> {}", line)).collect::<Vec<_>>().join("\n")
                ));
            }
            
            self.compose_email = reply;
            self.mode = AppMode::Compose;
            self.focus = FocusPanel::ComposeForm;
            
            self.show_info("Replying to email");
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
    }
    
    pub fn forward_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            let original = &self.emails[idx];
            
            let mut forward = Email::new();
            forward.subject = if original.subject.starts_with("Fwd: ") {
                original.subject.clone()
            } else {
                format!("Fwd: {}", original.subject)
            };
            
            // Set body with forwarded content
            if let Some(body) = &original.body_text {
                forward.body_text = Some(format!(
                    "\n\n---------- Forwarded message ----------\nFrom: {}\nDate: {}\nSubject: {}\nTo: {}\n\n{}",
                    original.from.iter().map(|addr| format!("{}", addr.address)).collect::<Vec<_>>().join(", "),
                    original.date.format("%Y-%m-%d %H:%M"),
                    original.subject,
                    original.to.iter().map(|addr| format!("{}", addr.address)).collect::<Vec<_>>().join(", "),
                    body
                ));
            }
            
            // Copy attachments
            forward.attachments = original.attachments.clone();
            
            self.compose_email = forward;
            self.mode = AppMode::Compose;
            self.focus = FocusPanel::ComposeForm;
            
            self.show_info("Forwarding email");
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
    }
    
    /// Delete the currently selected email using the current account
    pub fn delete_selected_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            let email = &self.emails[idx];
            
            // Get the current account's email client
            if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
                if let Some(client) = &account_data.email_client {
                    match client.delete_email(email) {
                        Ok(_) => {
                            self.emails.remove(idx);
                            
                            // Adjust selection after deletion
                            if self.emails.is_empty() {
                                self.selected_email_idx = None;
                            } else if idx >= self.emails.len() {
                                // If we deleted the last email, select the new last email
                                self.selected_email_idx = Some(self.emails.len() - 1);
                            }
                            // If we deleted an email in the middle, the selection stays the same
                            // which will now point to the next email
                            
                            self.show_info("Email deleted");
                        }
                        Err(e) => {
                            self.show_error(&format!("Failed to delete email: {}", e));
                            return Err(AppError::EmailError(e));
                        }
                    }
                } else {
                    self.show_error("Email client not initialized for current account");
                }
            } else {
                self.show_error("Current account not found");
            }
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
    }

    /// Send the composed email using the current account
    pub fn send_email(&mut self) -> AppResult<()> {
        // Get the current account's email client
        if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
            if let Some(client) = &account_data.email_client {
                // Set from address if not set
                if self.compose_email.from.is_empty() {
                    let account = &self.config.accounts[self.current_account_idx];
                    self.compose_email.from.push(crate::email::EmailAddress {
                        name: Some(account.name.clone()),
                        address: account.email.clone(),
                    });
                }
                
                match client.send_email(&self.compose_email) {
                    Ok(_) => {
                        self.show_info("Email sent successfully");
                        self.mode = AppMode::Normal;
                        self.focus = FocusPanel::EmailList;
                        Ok(())
                    }
                    Err(e) => {
                        self.show_error(&format!("Failed to send email: {}", e));
                        Err(AppError::EmailError(e))
                    }
                }
            } else {
                self.show_error("Email client not initialized for current account");
                Ok(())
            }
        } else {
            self.show_error("Current account not found");
            Ok(())
        }
    }

    pub fn show_error(&mut self, message: &str) {
        self.error_message = Some(message.to_string());
        self.message_timeout = Some(Instant::now() + Duration::from_secs(5));
    }
    
    pub fn show_info(&mut self, message: &str) {
        self.info_message = Some(message.to_string());
        self.message_timeout = Some(Instant::now() + Duration::from_secs(3));
    }
    
    pub fn tick(&mut self) -> AppResult<()> {
        // Clear messages after timeout
        if let Some(timeout) = self.message_timeout {
            if std::time::Instant::now() > timeout {
                self.error_message = None;
                self.info_message = None;
                self.message_timeout = None;
            }
        }
        
        Ok(())
    }
}


