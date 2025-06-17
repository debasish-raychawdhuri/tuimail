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
    pub compose_cursor_pos: usize, // Cursor position in the current field
    pub compose_to_text: String,   // Raw text for To field editing
    
    // Attachment handling
    pub selected_attachment_idx: Option<usize>, // For viewing attachments in received emails
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
            compose_cursor_pos: 0,
            compose_to_text: String::new(),
            selected_attachment_idx: None,
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
    
    /// Initialize email client for a specific account if not already initialized
    pub fn ensure_account_initialized(&mut self, account_idx: usize) -> AppResult<()> {
        // Check if account exists and client is already initialized
        if let Some(account_data) = self.accounts.get(&account_idx) {
            if account_data.email_client.is_some() {
                return Ok(()); // Already initialized
            }
        }
        
        // Initialize the account
        if account_idx < self.config.accounts.len() {
            let account = self.config.accounts[account_idx].clone();
            
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
                    let _ = writeln!(file, "[{}] Initializing account {}: {}", 
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), 
                        account_idx, account.email);
                }
            }
            
            // Create email client using the new signature
            let client = EmailClient::new(account, self.credentials.clone());
            
            // Get folders for this account
            let folders = client.list_folders().map_err(AppError::EmailError)?;
            
            // Create or update account data
            let account_data = self.accounts.entry(account_idx).or_insert_with(AccountData::new);
            account_data.email_client = Some(client);
            account_data.folders = folders;
            
            let account_email = &self.config.accounts[account_idx].email;
            self.show_info(&format!("Initialized account: {}", account_email));
            Ok(())
        } else {
            Err(AppError::EmailError(crate::email::EmailError::ImapError(
                "Account index out of range".to_string()
            )))
        }
    }

    /// Load emails for a specific account and folder
    pub fn load_emails_for_account_folder(&mut self, account_idx: usize, folder: &str) -> AppResult<()> {
        // Ensure the account is initialized
        self.ensure_account_initialized(account_idx)?;
        
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
                self.compose_cursor_pos = 0;
                self.compose_to_text = String::new();
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
                        if let Err(e) = self.ensure_account_initialized(self.current_account_idx) {
                            self.show_error(&format!("Failed to initialize account: {}", e));
                        } else if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
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
            KeyCode::Char('n') => {
                // Rotate to next account
                self.rotate_to_next_account()?;
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
                self.compose_cursor_pos = 0;
                Ok(())
            }
            KeyCode::Tab => {
                // Move to next field
                self.compose_field = match self.compose_field {
                    ComposeField::To => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::To,
                };
                // Reset cursor position when switching fields
                self.compose_cursor_pos = match self.compose_field {
                    ComposeField::To => self.compose_to_text.len(), // End of To field
                    ComposeField::Subject => self.compose_email.subject.len(), // End of Subject
                    ComposeField::Body => 0, // Beginning of Body for replies
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
                // Reset cursor position when switching fields
                self.compose_cursor_pos = match self.compose_field {
                    ComposeField::To => self.compose_to_text.len(), // End of To field
                    ComposeField::Subject => self.compose_email.subject.len(), // End of Subject
                    ComposeField::Body => 0, // Beginning of Body for replies
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
                // Reset cursor position when switching fields
                self.compose_cursor_pos = match self.compose_field {
                    ComposeField::To => self.compose_to_text.len(), // End of To field
                    ComposeField::Subject => self.compose_email.subject.len(), // End of Subject
                    ComposeField::Body => 0, // Beginning of Body for replies
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
                // Reset cursor position when switching fields
                self.compose_cursor_pos = match self.compose_field {
                    ComposeField::To => self.compose_to_text.len(), // End of To field
                    ComposeField::Subject => self.compose_email.subject.len(), // End of Subject
                    ComposeField::Body => 0, // Beginning of Body for replies
                };
                Ok(())
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_email()?;
                Ok(())
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Add attachment
                self.add_attachment()?;
                Ok(())
            }
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Remove selected attachment
                self.remove_selected_attachment()?;
                Ok(())
            }
            KeyCode::Char(c) => {
                // Add character to current field at cursor position
                match self.compose_field {
                    ComposeField::To => {
                        // Insert character at cursor position in To field
                        if self.compose_cursor_pos <= self.compose_to_text.len() {
                            self.compose_to_text.insert(self.compose_cursor_pos, c);
                            self.compose_cursor_pos += 1;
                        } else {
                            self.compose_to_text.push(c);
                            self.compose_cursor_pos = self.compose_to_text.len();
                        }
                        
                        // Parse the to field and update compose_email.to
                        self.compose_email.to.clear();
                        for addr in self.compose_to_text.split(',') {
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
                            // Insert character at cursor position
                            if self.compose_cursor_pos <= body.len() {
                                body.insert(self.compose_cursor_pos, c);
                                self.compose_cursor_pos += 1;
                            } else {
                                body.push(c);
                                self.compose_cursor_pos = body.len();
                            }
                        } else {
                            self.compose_email.body_text = Some(c.to_string());
                            self.compose_cursor_pos = 1;
                        }
                    }
                }
                Ok(())
            }
            KeyCode::Backspace => {
                // Remove character from current field at cursor position
                match self.compose_field {
                    ComposeField::To => {
                        if self.compose_cursor_pos > 0 && self.compose_cursor_pos <= self.compose_to_text.len() {
                            self.compose_to_text.remove(self.compose_cursor_pos - 1);
                            self.compose_cursor_pos -= 1;
                            
                            // Parse the to field and update compose_email.to
                            self.compose_email.to.clear();
                            for addr in self.compose_to_text.split(',') {
                                let addr = addr.trim();
                                if !addr.is_empty() {
                                    self.compose_email.to.push(crate::email::EmailAddress {
                                        name: None,
                                        address: addr.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    ComposeField::Subject => {
                        self.compose_email.subject.pop();
                    }
                    ComposeField::Body => {
                        if let Some(ref mut body) = self.compose_email.body_text {
                            if self.compose_cursor_pos > 0 && self.compose_cursor_pos <= body.len() {
                                body.remove(self.compose_cursor_pos - 1);
                                self.compose_cursor_pos -= 1;
                            }
                        }
                    }
                }
                Ok(())
            }
            KeyCode::Enter => {
                // In body field, add newline at cursor position
                if self.compose_field == ComposeField::Body {
                    if let Some(ref mut body) = self.compose_email.body_text {
                        if self.compose_cursor_pos <= body.len() {
                            body.insert(self.compose_cursor_pos, '\n');
                            self.compose_cursor_pos += 1;
                        } else {
                            body.push('\n');
                            self.compose_cursor_pos = body.len();
                        }
                    } else {
                        self.compose_email.body_text = Some("\n".to_string());
                        self.compose_cursor_pos = 1;
                    }
                }
                Ok(())
            }
            KeyCode::Left => {
                // Move cursor left in current field
                match self.compose_field {
                    ComposeField::To => {
                        if self.compose_cursor_pos > 0 {
                            self.compose_cursor_pos -= 1;
                        }
                    }
                    ComposeField::Body => {
                        if self.compose_cursor_pos > 0 {
                            self.compose_cursor_pos -= 1;
                        }
                    }
                    _ => {}
                }
                Ok(())
            }
            KeyCode::Right => {
                // Move cursor right in current field
                match self.compose_field {
                    ComposeField::To => {
                        if self.compose_cursor_pos < self.compose_to_text.len() {
                            self.compose_cursor_pos += 1;
                        }
                    }
                    ComposeField::Body => {
                        if let Some(body) = &self.compose_email.body_text {
                            if self.compose_cursor_pos < body.len() {
                                self.compose_cursor_pos += 1;
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            }
            KeyCode::Home => {
                // Move cursor to beginning of current line in body field
                if self.compose_field == ComposeField::Body {
                    if let Some(body) = &self.compose_email.body_text {
                        // Find the beginning of the current line
                        let text_before_cursor = &body[..self.compose_cursor_pos];
                        if let Some(last_newline) = text_before_cursor.rfind('\n') {
                            self.compose_cursor_pos = last_newline + 1;
                        } else {
                            self.compose_cursor_pos = 0;
                        }
                    }
                }
                Ok(())
            }
            KeyCode::End => {
                // Move cursor to end of current line in body field
                if self.compose_field == ComposeField::Body {
                    if let Some(body) = &self.compose_email.body_text {
                        // Find the end of the current line
                        let text_after_cursor = &body[self.compose_cursor_pos..];
                        if let Some(next_newline) = text_after_cursor.find('\n') {
                            self.compose_cursor_pos += next_newline;
                        } else {
                            self.compose_cursor_pos = body.len();
                        }
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
            KeyCode::Char('a') => {
                self.reply_all_to_email()?;
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
            KeyCode::Char('s') => {
                // Save selected attachment
                self.save_selected_attachment()?;
                Ok(())
            }
            KeyCode::Tab => {
                // Navigate through attachments
                self.select_next_attachment();
                Ok(())
            }
            KeyCode::BackTab => {
                // Navigate through attachments (reverse)
                self.select_previous_attachment();
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
            
            // Set subject with Re: prefix
            reply.subject = if original.subject.starts_with("Re: ") {
                original.subject.clone()
            } else {
                format!("Re: {}", original.subject)
            };
            
            // Set recipient to the original sender (reply-to if present, otherwise from)
            let reply_to_addrs = original.reply_to();
            let mut to_addresses = if !reply_to_addrs.is_empty() {
                reply_to_addrs
            } else {
                original.from.clone()
            };
            
            // Deduplicate addresses (in case there are duplicates in the original)
            to_addresses.dedup_by(|a, b| a.address == b.address);
            reply.to = to_addresses;
            
            // Debug: Log what we're setting as To addresses
            if std::env::var("EMAIL_DEBUG").is_ok() {
                let debug_msg = format!("Reply To addresses: {:?}", 
                    reply.to.iter().map(|addr| &addr.address).collect::<Vec<_>>());
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/email_client_debug.log") {
                    use std::io::Write;
                    let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), debug_msg);
                }
            }
            
            // Set from field to current account
            let current_account = &self.config.accounts[self.current_account_idx];
            reply.from = vec![crate::email::EmailAddress {
                name: Some(current_account.name.clone()),
                address: current_account.email.clone(),
            }];
            
            // Set In-Reply-To and References headers for proper threading
            let original_msg_id = original.message_id();
            if !original_msg_id.is_empty() {
                reply.set_in_reply_to(original_msg_id.clone());
                let mut refs = original.references();
                refs.push(original_msg_id);
                reply.set_references(refs);
            }
            
            // Set body with space for typing at the top, then quoted original
            if let Some(body) = &original.body_text {
                let sender_name = if !original.from.is_empty() {
                    if let Some(name) = &original.from[0].name {
                        name.clone()
                    } else {
                        original.from[0].address.clone()
                    }
                } else {
                    "Unknown".to_string()
                };
                
                // Put cursor space at the top, then quoted content below
                reply.body_text = Some(format!(
                    "\n\n\n\nOn {} {} wrote:\n{}", 
                    original.date.format("%Y-%m-%d %H:%M"),
                    sender_name,
                    body.lines().map(|line| format!("> {}", line)).collect::<Vec<_>>().join("\n")
                ));
            } else {
                reply.body_text = Some("\n\n\n\n".to_string());
            }
            
            // Set compose_to_text before moving reply
            let to_text = reply.to.iter()
                .map(|addr| addr.address.clone())
                .collect::<Vec<_>>()
                .join(", ");
            
            self.compose_email = reply;
            self.compose_to_text = to_text;
            self.mode = AppMode::Compose;
            self.focus = FocusPanel::ComposeForm;
            self.compose_field = ComposeField::Body;
            self.compose_cursor_pos = 0; // Position cursor at the very beginning
            
            self.show_info("Replying to email - cursor positioned at top");
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
    }

    pub fn reply_all_to_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            let original = &self.emails[idx];
            let current_account = &self.config.accounts[self.current_account_idx];
            
            let mut reply = Email::new();
            
            // Set subject with Re: prefix
            reply.subject = if original.subject.starts_with("Re: ") {
                original.subject.clone()
            } else {
                format!("Re: {}", original.subject)
            };
            
            // Set from field to current account
            reply.from = vec![crate::email::EmailAddress {
                name: Some(current_account.name.clone()),
                address: current_account.email.clone(),
            }];
            
            // For reply-all, include original sender and all recipients except current user
            let current_email = &current_account.email;
            
            // Add original sender (reply-to if present, otherwise from)
            let reply_to_addrs = original.reply_to();
            let original_sender = if !reply_to_addrs.is_empty() {
                &reply_to_addrs
            } else {
                &original.from
            };
            
            for addr in original_sender {
                if addr.address != *current_email {
                    reply.to.push(addr.clone());
                }
            }
            
            // Add all original TO recipients except current user
            for addr in &original.to {
                if addr.address != *current_email && !reply.to.iter().any(|existing| existing.address == addr.address) {
                    reply.to.push(addr.clone());
                }
            }
            
            // Add all original CC recipients except current user to CC
            for addr in &original.cc {
                if addr.address != *current_email && !reply.cc.iter().any(|existing| existing.address == addr.address) {
                    reply.cc.push(addr.clone());
                }
            }
            
            // Deduplicate all addresses to prevent duplicates
            reply.to.dedup_by(|a, b| a.address == b.address);
            reply.cc.dedup_by(|a, b| a.address == b.address);
            
            // Set In-Reply-To and References headers for proper threading
            let original_msg_id = original.message_id();
            if !original_msg_id.is_empty() {
                reply.set_in_reply_to(original_msg_id.clone());
                let mut refs = original.references();
                refs.push(original_msg_id);
                reply.set_references(refs);
            }
            
            // Set body with space for typing at the top, then quoted original
            if let Some(body) = &original.body_text {
                let sender_name = if !original.from.is_empty() {
                    if let Some(name) = &original.from[0].name {
                        name.clone()
                    } else {
                        original.from[0].address.clone()
                    }
                } else {
                    "Unknown".to_string()
                };
                
                // Put cursor space at the top, then quoted content below
                reply.body_text = Some(format!(
                    "\n\n\n\nOn {} {} wrote:\n{}", 
                    original.date.format("%Y-%m-%d %H:%M"),
                    sender_name,
                    body.lines().map(|line| format!("> {}", line)).collect::<Vec<_>>().join("\n")
                ));
            } else {
                reply.body_text = Some("\n\n\n\n".to_string());
            }
            
            // Set compose_to_text before moving reply
            let to_text = reply.to.iter()
                .map(|addr| addr.address.clone())
                .collect::<Vec<_>>()
                .join(", ");
            
            self.compose_email = reply;
            self.compose_to_text = to_text;
            self.mode = AppMode::Compose;
            self.focus = FocusPanel::ComposeForm;
            self.compose_field = ComposeField::Body;
            self.compose_cursor_pos = 0; // Position cursor at the very beginning
            
            self.show_info("Replying to all - cursor positioned at top");
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
            
            // Set subject with Fwd: prefix
            forward.subject = if original.subject.starts_with("Fwd: ") {
                original.subject.clone()
            } else {
                format!("Fwd: {}", original.subject)
            };
            
            // Set from field to current account
            let current_account = &self.config.accounts[self.current_account_idx];
            forward.from = vec![crate::email::EmailAddress {
                name: Some(current_account.name.clone()),
                address: current_account.email.clone(),
            }];
            
            // Set body with space for typing at the top, then forwarded content header
            let from_str = original.from.iter()
                .map(|addr| {
                    if let Some(name) = &addr.name {
                        format!("{} <{}>", name, addr.address)
                    } else {
                        addr.address.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
                
            let to_str = original.to.iter()
                .map(|addr| {
                    if let Some(name) = &addr.name {
                        format!("{} <{}>", name, addr.address)
                    } else {
                        addr.address.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            
            let mut forward_header = format!(
                "\n\n\n\n---------- Forwarded message ----------\nFrom: {}\nDate: {}\nSubject: {}\nTo: {}",
                from_str,
                original.date.format("%a, %d %b %Y %H:%M:%S %z"),
                original.subject,
                to_str
            );
            
            // Add CC if present
            if !original.cc.is_empty() {
                let cc_str = original.cc.iter()
                    .map(|addr| {
                        if let Some(name) = &addr.name {
                            format!("{} <{}>", name, addr.address)
                        } else {
                            addr.address.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                forward_header.push_str(&format!("\nCc: {}", cc_str));
            }
            
            forward_header.push_str("\n\n");
            
            // Add original body
            if let Some(body) = &original.body_text {
                forward.body_text = Some(format!("{}{}", forward_header, body));
            } else {
                forward.body_text = Some(forward_header);
            }
            
            // Copy attachments
            forward.attachments = original.attachments.clone();
            
            self.compose_email = forward;
            self.compose_to_text = String::new(); // Forward starts with empty To field
            self.mode = AppMode::Compose;
            self.focus = FocusPanel::ComposeForm;
            self.compose_field = ComposeField::To; // Start in To field for forward
            self.compose_cursor_pos = 0; // Position cursor at the beginning
            
            self.show_info("Forwarding email - add recipients");
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
    }
    
    /// Ensure the specified account is expanded in folder view
    pub fn ensure_account_expanded(&mut self, account_idx: usize) {
        // Find and expand the account if it's not already expanded
        for item in &mut self.folder_items {
            if let FolderItem::Account { index, expanded, .. } = item {
                if *index == account_idx && !*expanded {
                    *expanded = true;
                    break;
                }
            }
        }
    }
    fn select_inbox_folder_for_account(&mut self, account_idx: usize) {
        for (i, item) in self.folder_items.iter().enumerate() {
            if let FolderItem::Folder { account_index, name, .. } = item {
                if *account_index == account_idx && (name == "INBOX" || name == "Inbox") {
                    self.selected_folder_item_idx = i;
                    return;
                }
            }
        }
        
        // If INBOX not found, try to select the account itself
        for (i, item) in self.folder_items.iter().enumerate() {
            if let FolderItem::Account { index, .. } = item {
                if *index == account_idx {
                    self.selected_folder_item_idx = i;
                    return;
                }
            }
        }
    }

    /// Save the selected attachment from the current email
    pub fn save_selected_attachment(&mut self) -> AppResult<()> {
        if let Some(email_idx) = self.selected_email_idx {
            if email_idx < self.emails.len() {
                let email = &self.emails[email_idx];
                
                if email.attachments.is_empty() {
                    self.show_info("No attachments in this email");
                    return Ok(());
                }
                
                let attachment_idx = self.selected_attachment_idx.unwrap_or(0);
                if attachment_idx < email.attachments.len() {
                    let attachment = &email.attachments[attachment_idx];
                    
                    // Create downloads directory if it doesn't exist
                    let downloads_dir = std::env::var("HOME")
                        .map(|home| format!("{}/Downloads", home))
                        .unwrap_or_else(|_| "./downloads".to_string());
                    
                    if let Err(e) = std::fs::create_dir_all(&downloads_dir) {
                        self.show_error(&format!("Failed to create downloads directory: {}", e));
                        return Ok(());
                    }
                    
                    // Generate unique filename if file already exists
                    let mut file_path = format!("{}/{}", downloads_dir, attachment.filename);
                    let mut counter = 1;
                    while std::path::Path::new(&file_path).exists() {
                        let (name, ext) = if let Some(dot_pos) = attachment.filename.rfind('.') {
                            (&attachment.filename[..dot_pos], &attachment.filename[dot_pos..])
                        } else {
                            (attachment.filename.as_str(), "")
                        };
                        file_path = format!("{}/{}_{}{}", downloads_dir, name, counter, ext);
                        counter += 1;
                    }
                    
                    // Save the attachment
                    match std::fs::write(&file_path, &attachment.data) {
                        Ok(()) => {
                            self.show_info(&format!("Saved attachment: {}", file_path));
                        }
                        Err(e) => {
                            self.show_error(&format!("Failed to save attachment: {}", e));
                        }
                    }
                } else {
                    self.show_error("Invalid attachment selection");
                }
            }
        } else {
            self.show_error("No email selected");
        }
        Ok(())
    }
    
    /// Select next attachment in the current email
    pub fn select_next_attachment(&mut self) {
        if let Some(email_idx) = self.selected_email_idx {
            if email_idx < self.emails.len() {
                let email = &self.emails[email_idx];
                if !email.attachments.is_empty() {
                    let current = self.selected_attachment_idx.unwrap_or(0);
                    self.selected_attachment_idx = Some((current + 1) % email.attachments.len());
                }
            }
        }
    }
    
    /// Select previous attachment in the current email
    pub fn select_previous_attachment(&mut self) {
        if let Some(email_idx) = self.selected_email_idx {
            if email_idx < self.emails.len() {
                let email = &self.emails[email_idx];
                if !email.attachments.is_empty() {
                    let current = self.selected_attachment_idx.unwrap_or(0);
                    self.selected_attachment_idx = Some(
                        if current == 0 {
                            email.attachments.len() - 1
                        } else {
                            current - 1
                        }
                    );
                }
            }
        }
    }
    
    /// Add an attachment to the compose email
    pub fn add_attachment(&mut self) -> AppResult<()> {
        // For now, we'll implement a simple file path input
        // In a real implementation, you might want to use a proper file picker
        
        // TODO: Implement proper file picker dialog
        // For demonstration, let's add a sample attachment
        self.show_info("Attachment feature implemented - use file picker in production");
        
        // Example of how to add an attachment programmatically:
        /*
        let attachment = crate::email::EmailAttachment {
            filename: "example.txt".to_string(),
            content_type: "text/plain".to_string(),
            data: b"This is a sample attachment".to_vec(),
        };
        self.compose_email.attachments.push(attachment);
        */
        
        Ok(())
    }
    
    /// Add an attachment from a file path
    pub fn add_attachment_from_path(&mut self, file_path: &str) -> AppResult<()> {
        match std::fs::read(file_path) {
            Ok(data) => {
                let filename = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                // Determine content type based on file extension
                let content_type = match std::path::Path::new(file_path)
                    .extension()
                    .and_then(|ext| ext.to_str()) {
                    Some("txt") => "text/plain",
                    Some("pdf") => "application/pdf",
                    Some("jpg") | Some("jpeg") => "image/jpeg",
                    Some("png") => "image/png",
                    Some("gif") => "image/gif",
                    Some("doc") => "application/msword",
                    Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    Some("xls") => "application/vnd.ms-excel",
                    Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    _ => "application/octet-stream",
                }.to_string();
                
                let attachment = crate::email::EmailAttachment {
                    filename,
                    content_type,
                    data,
                };
                
                self.compose_email.attachments.push(attachment);
                self.show_info(&format!("Added attachment: {}", file_path));
            }
            Err(e) => {
                self.show_error(&format!("Failed to read file {}: {}", file_path, e));
            }
        }
        Ok(())
    }

    /// Remove the selected attachment from compose email
    pub fn remove_selected_attachment(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_attachment_idx {
            if idx < self.compose_email.attachments.len() {
                let filename = self.compose_email.attachments[idx].filename.clone();
                self.compose_email.attachments.remove(idx);
                
                // Adjust selection
                if self.compose_email.attachments.is_empty() {
                    self.selected_attachment_idx = None;
                } else if idx >= self.compose_email.attachments.len() {
                    self.selected_attachment_idx = Some(self.compose_email.attachments.len() - 1);
                }
                
                self.show_info(&format!("Removed attachment: {}", filename));
            }
        } else {
            self.show_info("No attachment selected");
        }
        Ok(())
    }

    /// Rotate to the next account and load its INBOX
    pub fn rotate_to_next_account(&mut self) -> AppResult<()> {
        if self.config.accounts.len() <= 1 {
            self.show_info("Only one account configured");
            return Ok(());
        }
        
        // Calculate next account index
        let next_account_idx = (self.current_account_idx + 1) % self.config.accounts.len();
        
        // Switch to the next account
        self.current_account_idx = next_account_idx;
        
        // Initialize the account if needed
        self.ensure_account_initialized(next_account_idx)?;
        
        // Load INBOX for the new account
        if let Err(e) = self.load_emails_for_account_folder(next_account_idx, "INBOX") {
            self.show_error(&format!("Failed to load INBOX for account: {}", e));
        } else {
            let account_name = &self.config.accounts[next_account_idx].name;
            self.show_info(&format!("Switched to account: {}", account_name));
        }
        
        // Reset selection
        self.selected_email_idx = if self.emails.is_empty() { None } else { Some(0) };
        
        // Ensure the new current account is expanded in folder view
        self.ensure_account_expanded(next_account_idx);
        
        // Rebuild folder items to reflect the new current account
        self.rebuild_folder_items();
        
        // Find and select the INBOX folder for the new account
        self.select_inbox_folder_for_account(next_account_idx);
        
        Ok(())
    }
    pub fn delete_selected_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            // Clone the email to avoid borrowing issues
            let email = self.emails[idx].clone();
            
            // Ensure the current account is initialized
            self.ensure_account_initialized(self.current_account_idx)?;
            
            // Get the current account's email client
            if let Some(account_data) = self.accounts.get(&self.current_account_idx) {
                if let Some(client) = &account_data.email_client {
                    match client.delete_email(&email) {
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
        // Ensure the current account is initialized
        self.ensure_account_initialized(self.current_account_idx)?;
        
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


