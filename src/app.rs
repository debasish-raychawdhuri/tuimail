use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use thiserror::Error;

use crate::config::Config;
use crate::email::{Email, EmailClient, EmailFetcher};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Email error: {0}")]
    EmailError(#[from] crate::email::EmailError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type AppResult<T> = std::result::Result<T, AppError>;

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
    FolderList,
    EmailList,
    EmailView,
    ComposeForm,
}

pub struct App {
    pub config: Config,
    pub should_quit: bool,
    pub mode: AppMode,
    pub focus: FocusPanel,
    pub emails: Vec<Email>,
    pub folders: Vec<String>,
    pub selected_folder_idx: usize,
    pub selected_email_idx: Option<usize>,
    pub compose_email: Email,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub message_timeout: Option<Instant>,
    pub email_client: Option<EmailClient>,
    pub email_rx: Option<mpsc::Receiver<Vec<Email>>>,
    pub email_fetcher: Option<EmailFetcher>,
    pub last_tick: Instant,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            should_quit: false,
            mode: AppMode::Normal,
            focus: FocusPanel::EmailList,
            emails: Vec::new(),
            folders: vec!["INBOX".to_string()],
            selected_folder_idx: 0,
            selected_email_idx: None,
            compose_email: Email::new(),
            error_message: None,
            info_message: None,
            message_timeout: None,
            email_client: None,
            email_rx: None,
            email_fetcher: None,
            last_tick: Instant::now(),
        }
    }
    
    pub fn init(&mut self) -> AppResult<()> {
        // Validate that we have accounts configured
        if self.config.accounts.is_empty() {
            return Err(AppError::EmailError(
                crate::email::EmailError::ImapError("No email accounts configured".to_string())
            ));
        }
        
        // Create email client with bounds checking
        let account = match self.config.get_current_account() {
            Ok(account) => account.clone(),
            Err(e) => {
                self.show_error(&format!("Configuration error: {}", e));
                return Err(AppError::EmailError(
                    crate::email::EmailError::ImapError(e.to_string())
                ));
            }
        };
        
        let email_client = EmailClient::new(account);
        
        // Set up channels for email fetching
        let (tx, rx) = mpsc::channel(100);
        
        // Create and start email fetcher
        let mut fetcher = EmailFetcher::new(
            email_client.clone(),
            tx,
            self.config.ui.refresh_interval,
        );
        fetcher.start();
        
        // Store components
        self.email_client = Some(email_client);
        self.email_rx = Some(rx);
        self.email_fetcher = Some(fetcher);
        
        // Load folders with error handling
        if let Err(e) = self.load_folders() {
            self.show_error(&format!("Failed to load folders: {}", e));
            // Continue with default folders
            self.folders = vec!["INBOX".to_string()];
        }
        
        // Load initial emails with error handling
        if let Err(e) = self.load_emails() {
            self.show_error(&format!("Failed to load emails: {}", e));
            // Continue with empty email list
            self.emails = Vec::new();
        }
        
        Ok(())
    }
    
    pub fn load_folders(&mut self) -> AppResult<()> {
        if let Some(client) = &self.email_client {
            match client.list_folders() {
                Ok(folders) => {
                    self.folders = folders;
                    Ok(())
                }
                Err(e) => {
                    self.show_error(&format!("Failed to load folders: {}", e));
                    Err(AppError::EmailError(e))
                }
            }
        } else {
            self.show_error("Email client not initialized");
            Ok(())
        }
    }
    
    pub fn load_emails(&mut self) -> AppResult<()> {
        if let Some(client) = &self.email_client {
            let folder = &self.folders[self.selected_folder_idx];
            match client.fetch_emails(folder, 50) {
                Ok(emails) => {
                    self.emails = emails;
                    Ok(())
                }
                Err(e) => {
                    self.show_error(&format!("Failed to load emails: {}", e));
                    Err(AppError::EmailError(e))
                }
            }
        } else {
            self.show_error("Email client not initialized");
            Ok(())
        }
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
                Ok(())
            }
            KeyCode::Char('r') => {
                self.load_emails()?;
                self.show_info("Emails refreshed");
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
                        if let Some(client) = &self.email_client {
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
                Ok(())
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_email()?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn handle_view_mode(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
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
                self.focus = FocusPanel::EmailList;
                Ok(())
            }
            KeyCode::Up => {
                if !self.folders.is_empty() && self.selected_folder_idx > 0 {
                    self.selected_folder_idx -= 1;
                }
                Ok(())
            }
            KeyCode::Down => {
                if !self.folders.is_empty() && self.selected_folder_idx < self.folders.len().saturating_sub(1) {
                    self.selected_folder_idx += 1;
                }
                Ok(())
            }
            KeyCode::Enter => {
                if self.selected_folder_idx < self.folders.len() {
                    self.mode = AppMode::Normal;
                    self.focus = FocusPanel::EmailList;
                    if let Err(e) = self.load_emails() {
                        self.show_error(&format!("Failed to load emails: {}", e));
                    }
                } else {
                    self.show_error("Invalid folder selection");
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
    
    pub fn delete_selected_email(&mut self) -> AppResult<()> {
        if let Some(idx) = self.selected_email_idx {
            if idx >= self.emails.len() {
                self.show_error("Invalid email selection");
                return Ok(());
            }
            
            let email = &self.emails[idx];
            
            if let Some(client) = &self.email_client {
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
                self.show_error("Email client not initialized");
            }
        } else {
            self.show_error("No email selected");
        }
        
        Ok(())
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
    
    pub fn send_email(&mut self) -> AppResult<()> {
        if let Some(client) = &self.email_client {
            // Set from address if not set
            if self.compose_email.from.is_empty() {
                let account = self.config.get_current_account_safe();
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
            self.show_error("Email client not initialized");
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
        // Check for new emails from the background fetcher
        if let Some(rx) = &mut self.email_rx {
            match rx.try_recv() {
                Ok(emails) => {
                    // Update emails and maintain selection if possible
                    let old_selection = self.selected_email_idx;
                    self.emails = emails;
                    
                    // Try to maintain selection after update
                    if let Some(old_idx) = old_selection {
                        if old_idx >= self.emails.len() {
                            // If old selection is out of bounds, select the last email
                            self.selected_email_idx = if self.emails.is_empty() {
                                None
                            } else {
                                Some(self.emails.len() - 1)
                            };
                        }
                        // Otherwise keep the same selection
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No new emails, this is normal
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Background fetcher has stopped, this might indicate an error
                    self.show_error("Background email fetcher disconnected");
                    self.email_rx = None; // Clear the receiver to avoid repeated errors
                }
            }
        }
        
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

impl Drop for App {
    fn drop(&mut self) {
        // Stop the email fetcher if it exists
        if let Some(mut fetcher) = self.email_fetcher.take() {
            fetcher.stop();
        }
    }
}
