use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fs;

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use imap::Session;
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use native_tls::{TlsConnector, TlsStream};
use thiserror::Error;
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};

use crate::config::{EmailAccount, ImapSecurity, SmtpSecurity};

// Helper function to parse email addresses from header values
fn parse_email_addresses(value: &str) -> Vec<EmailAddress> {
    let mut addresses = Vec::new();
    
    // Handle multiple addresses separated by commas
    for addr_part in value.split(',') {
        let addr_part = addr_part.trim();
        if addr_part.is_empty() {
            continue;
        }
        
        // Handle different formats:
        // 1. "Name" <email@domain.com>
        // 2. Name <email@domain.com>
        // 3. <email@domain.com>
        // 4. email@domain.com
        
        if let Some(addr_start) = addr_part.find('<') {
            if let Some(addr_end) = addr_part.find('>') {
                let email_addr = &addr_part[addr_start + 1..addr_end];
                let name_part = addr_part[..addr_start].trim();
                
                // Remove quotes from name if present
                let clean_name = if name_part.starts_with('"') && name_part.ends_with('"') {
                    &name_part[1..name_part.len()-1]
                } else {
                    name_part
                };
                
                addresses.push(EmailAddress {
                    name: if clean_name.is_empty() { None } else { Some(clean_name.to_string()) },
                    address: email_addr.to_string(),
                });
            }
        } else if addr_part.contains('@') {
            // No angle brackets, assume the whole thing is an email
            addresses.push(EmailAddress {
                name: None,
                address: addr_part.to_string(),
            });
        }
    }
    
    addresses
}

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("IMAP error: {0}")]
    ImapError(String),
    
    #[error("SMTP error: {0}")]
    SmtpError(String),
    
    #[error("TLS error: {0}")]
    TlsError(#[from] native_tls::Error),
    
    #[error("Parsing error: {0}")]
    ParsingError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: String,
}

impl From<EmailAddress> for Mailbox {
    fn from(addr: EmailAddress) -> Self {
        let parts: Vec<&str> = addr.address.split('@').collect();
        if parts.len() == 2 {
            let address = Address::new(parts[0], parts[1]).unwrap_or_else(|_| {
                // Fallback to a safe default if parsing fails
                Address::new("unknown", "example.com").unwrap()
            });
            match addr.name {
                Some(name) => Mailbox::new(Some(name), address),
                None => Mailbox::new(None, address),
            }
        } else {
            // Fallback for invalid email format
            let address = Address::new("unknown", "example.com").unwrap();
            match addr.name {
                Some(name) => Mailbox::new(Some(name), address),
                None => Mailbox::new(None, address),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: String,
    pub subject: String,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    #[serde(with = "local_datetime_serde")]
    pub date: DateTime<Local>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<EmailAttachment>,
    pub flags: Vec<String>,
    pub headers: HashMap<String, String>,
    pub seen: bool,
    pub folder: String,
}

// Custom serialization for DateTime<Local>
mod local_datetime_serde {
    use chrono::{DateTime, Local, TimeZone};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(dt: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        dt.timestamp().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = i64::deserialize(deserializer)?;
        Ok(Local.timestamp_opt(timestamp, 0).single().unwrap_or_else(|| Local::now()))
    }
}

impl Email {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            subject: String::new(),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            date: Local::now(),
            body_text: None,
            body_html: None,
            attachments: Vec::new(),
            flags: Vec::new(),
            headers: HashMap::new(),
            seen: false,
            folder: "INBOX".to_string(),
        }
    }
    
    pub fn from_parsed_email(parsed: &mail_parser::Message, id: &str, folder: &str, flags: Vec<String>) -> Result<Self, EmailError> {
        let mut email = Email::new();
        
        email.id = id.to_string();
        email.folder = folder.to_string();
        email.flags = flags;
        email.seen = email.flags.iter().any(|f| f == "\\Seen");
        
        // Extract subject
        email.subject = parsed.subject().unwrap_or_default().to_string();
        
        // Extract date
        if let Some(date) = parsed.date() {
            email.date = DateTime::from_timestamp(date.to_timestamp(), 0)
                .unwrap_or_else(|| Utc::now())
                .with_timezone(&Local);
        }
        
        // Extract headers and parse addresses from them
        for header in parsed.headers() {
            let name = header.name().to_string();
            if let Some(value) = header.value().as_text_ref() {
                email.headers.insert(name.clone(), value.to_string());
                
                // Parse basic from/to information from headers
                match name.to_lowercase().as_str() {
                    "from" => {
                        let addresses = parse_email_addresses(value);
                        email.from.extend(addresses);
                    }
                    "to" => {
                        let addresses = parse_email_addresses(value);
                        email.to.extend(addresses);
                    }
                    "cc" => {
                        let addresses = parse_email_addresses(value);
                        email.cc.extend(addresses);
                    }
                    _ => {}
                }
            }
        }
        
        // If we still don't have a from address, try to create one from the headers map
        if email.from.is_empty() {
            if let Some(from_value) = email.headers.get("From").or_else(|| email.headers.get("from")) {
                let addresses = parse_email_addresses(from_value);
                email.from.extend(addresses);
            }
        }
        
        // Extract body parts
        if let Some(text_body) = parsed.body_text(0) {
            email.body_text = Some(text_body.to_string());
        }
        
        if let Some(html_body) = parsed.body_html(0) {
            email.body_html = Some(html_body.to_string());
        }
        
        Ok(email)
    }
}

#[derive(Clone)]
pub struct EmailClient {
    account: EmailAccount,
    cache_dir: String,
}

impl EmailClient {
    pub fn new(account: EmailAccount) -> Self {
        let cache_dir = format!("{}/.cache/email_client/{}", 
            dirs::home_dir().unwrap_or_default().display(), 
            account.email.replace('@', "_at_").replace('.', "_"));
        
        // Create cache directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("Warning: Could not create cache directory {}: {}", cache_dir, e);
        }
        
        Self { account, cache_dir }
    }
    
    fn get_cache_file(&self, folder: &str) -> String {
        format!("{}/{}.json", self.cache_dir, folder.replace('/', "_"))
    }
    
    fn load_cached_emails(&self, folder: &str) -> Vec<Email> {
        let cache_file = self.get_cache_file(folder);
        if let Ok(content) = fs::read_to_string(&cache_file) {
            if let Ok(emails) = serde_json::from_str::<Vec<Email>>(&content) {
                return emails;
            }
        }
        Vec::new()
    }
    
    fn save_cached_emails(&self, folder: &str, emails: &[Email]) {
        let cache_file = self.get_cache_file(folder);
        if let Ok(content) = serde_json::to_string_pretty(emails) {
            if let Err(e) = fs::write(&cache_file, content) {
                eprintln!("Warning: Could not save email cache: {}", e);
            }
        }
    }
    
    fn merge_emails(&self, cached: Vec<Email>, new: Vec<Email>) -> Vec<Email> {
        let mut email_map: HashMap<String, Email> = HashMap::new();
        
        // Add cached emails first
        for email in cached {
            email_map.insert(email.id.clone(), email);
        }
        
        // Add/update with new emails
        for email in new {
            email_map.insert(email.id.clone(), email);
        }
        
        // Convert back to vector and sort by date
        let mut emails: Vec<Email> = email_map.into_values().collect();
        emails.sort_by(|a, b| b.date.cmp(&a.date));
        
        emails
    }
    
    fn connect_imap_secure(&self) -> Result<Session<TlsStream<std::net::TcpStream>>, EmailError> {
        let domain = &self.account.imap_server;
        let port = self.account.imap_port;
        let username = &self.account.imap_username;
        let password = &self.account.imap_password;
        
        let tls = TlsConnector::builder().build()?;
        let client = imap::connect((domain.as_str(), port), domain, &tls)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let session = client
            .login(username, password)
            .map_err(|e| EmailError::ImapError(e.0.to_string()))?;
        
        Ok(session)
    }
    
    fn connect_imap_plain(&self) -> Result<Session<std::net::TcpStream>, EmailError> {
        let domain = &self.account.imap_server;
        let port = self.account.imap_port;
        let username = &self.account.imap_username;
        let password = &self.account.imap_password;
        
        let tcp_stream = std::net::TcpStream::connect((domain.as_str(), port))
            .map_err(|e| EmailError::IoError(e))?;
        
        let client = imap::Client::new(tcp_stream);
        let session = client
            .login(username, password)
            .map_err(|e| EmailError::ImapError(e.0.to_string()))?;
        
        Ok(session)
    }
    
    pub fn list_folders(&self) -> Result<Vec<String>, EmailError> {
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                let folders = session
                    .list(None, Some("*"))
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                let folder_names = folders
                    .iter()
                    .map(|f| String::from_utf8_lossy(f.name().as_bytes()).into_owned())
                    .collect();
                
                Ok(folder_names)
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                let folders = session
                    .list(None, Some("*"))
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                let folder_names = folders
                    .iter()
                    .map(|f| String::from_utf8_lossy(f.name().as_bytes()).into_owned())
                    .collect();
                
                Ok(folder_names)
            }
        }
    }
    
    pub fn fetch_emails(&self, folder: &str, limit: usize) -> Result<Vec<Email>, EmailError> {
        // Load cached emails first
        let cached_emails = self.load_cached_emails(folder);
        
        // Fetch new emails from server
        let new_emails = match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                self.fetch_emails_from_server_secure(folder, limit)
            }
            ImapSecurity::None => {
                self.fetch_emails_from_server_plain(folder, limit)
            }
        };
        
        match new_emails {
            Ok(new) => {
                // Merge cached and new emails
                let merged = self.merge_emails(cached_emails, new);
                
                // Save updated cache
                self.save_cached_emails(folder, &merged);
                
                // Return limited number for display, but keep all in cache
                let display_limit = std::cmp::min(limit * 3, merged.len()); // Show more than requested
                Ok(merged.into_iter().take(display_limit).collect())
            }
            Err(e) => {
                // If server fetch fails, return cached emails
                if !cached_emails.is_empty() {
                    eprintln!("Using cached emails due to server error: {}", e);
                    Ok(cached_emails)
                } else {
                    Err(e)
                }
            }
        }
    }
    
    fn fetch_emails_from_server_secure(&self, folder: &str, limit: usize) -> Result<Vec<Email>, EmailError> {
        let mut session = self.connect_imap_secure()?;
        
        session
            .select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let message_ids: Vec<u32> = session
            .search("ALL")
            .map_err(|e| EmailError::ImapError(e.to_string()))?
            .into_iter()
            .collect();
        
        let message_count = message_ids.len();
        let start_idx = if message_count > limit {
            message_count - limit
        } else {
            0
        };
        
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        
        let sequence = if start_idx < message_count {
            format!("{}:*", message_ids[start_idx])
        } else {
            return Ok(Vec::new());
        };
        
        let messages = session
            .fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        self.parse_messages(&messages, folder)
    }
    
    fn fetch_emails_from_server_plain(&self, folder: &str, limit: usize) -> Result<Vec<Email>, EmailError> {
        let mut session = self.connect_imap_plain()?;
        
        session
            .select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let message_ids: Vec<u32> = session
            .search("ALL")
            .map_err(|e| EmailError::ImapError(e.to_string()))?
            .into_iter()
            .collect();
        
        let message_count = message_ids.len();
        let start_idx = if message_count > limit {
            message_count - limit
        } else {
            0
        };
        
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        
        let sequence = if start_idx < message_count {
            format!("{}:*", message_ids[start_idx])
        } else {
            return Ok(Vec::new());
        };
        
        let messages = session
            .fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        self.parse_messages(&messages, folder)
    }
    
    fn parse_messages(&self, messages: &[imap::types::Fetch], folder: &str) -> Result<Vec<Email>, EmailError> {
        let mut emails = Vec::new();
        
        for message in messages.iter() {
            if let Some(body) = message.body() {
                let uid = message.uid.unwrap_or(0).to_string();
                
                let flags: Vec<String> = message
                    .flags()
                    .iter()
                    .map(|f| f.to_string())
                    .collect();
                
                // Debug: Print first few bytes of the email to see if we're getting data
                if std::env::var("EMAIL_DEBUG").is_ok() {
                    eprintln!("Parsing email UID {}, body length: {}", uid, body.len());
                    if body.len() > 100 {
                        eprintln!("First 200 chars: {}", String::from_utf8_lossy(&body[..200]));
                    }
                }
                
                match mail_parser::Message::parse(body) {
                    Some(parsed) => {
                        match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                            Ok(mut email) => {
                                // Debug output if enabled
                                if std::env::var("EMAIL_DEBUG").is_ok() {
                                    eprintln!("Parsed email: subject='{}', from={:?}", email.subject, email.from);
                                }
                                
                                // Fallback: if we still don't have from addresses, try to extract from raw headers
                                if email.from.is_empty() {
                                    if let Some(from_header) = email.headers.get("From").or_else(|| email.headers.get("from")) {
                                        let addresses = parse_email_addresses(from_header);
                                        email.from.extend(addresses);
                                    }
                                }
                                
                                emails.push(email);
                            }
                            Err(e) => eprintln!("Error parsing email: {}", e),
                        }
                    }
                    None => {
                        eprintln!("Failed to parse email with mail_parser");
                    }
                }
            }
        }
        
        // Sort by date, newest first
        emails.sort_by(|a, b| b.date.cmp(&a.date));
        
        Ok(emails)
    }
    
    pub fn send_email(&self, email: &Email) -> Result<(), EmailError> {
        let mut message_builder = Message::builder()
            .subject(&email.subject);
        
        // Add From
        if let Some(from) = email.from.first() {
            message_builder = message_builder.from(from.clone().into());
        } else {
            // Use account email if no from address is specified
            let from = EmailAddress {
                name: Some(self.account.name.clone()),
                address: self.account.email.clone(),
            };
            message_builder = message_builder.from(from.into());
        }
        
        // Add To
        for to in &email.to {
            message_builder = message_builder.to(to.clone().into());
        }
        
        // Add CC
        for cc in &email.cc {
            message_builder = message_builder.cc(cc.clone().into());
        }
        
        // Add BCC
        for bcc in &email.bcc {
            message_builder = message_builder.bcc(bcc.clone().into());
        }
        
        // Build the email body
        let multipart = MultiPart::alternative()
            .singlepart(
                SinglePart::plain(email.body_text.clone().unwrap_or_default())
            );
        
        // Build the final message
        let message = message_builder
            .multipart(multipart)
            .map_err(|e| EmailError::SmtpError(e.to_string()))?;
        
        // Configure SMTP transport
        let creds = Credentials::new(
            self.account.smtp_username.clone(),
            self.account.smtp_password.clone(),
        );
        
        let mailer = match self.account.smtp_security {
            SmtpSecurity::SSL => {
                let tls_params = lettre::transport::smtp::client::TlsParameters::new(self.account.smtp_server.clone())
                    .map_err(|e| EmailError::SmtpError(e.to_string()))?;
                
                SmtpTransport::relay(&self.account.smtp_server)
                    .map_err(|e| EmailError::SmtpError(e.to_string()))?
                    .credentials(creds)
                    .port(self.account.smtp_port)
                    .tls(lettre::transport::smtp::client::Tls::Wrapper(tls_params))
                    .build()
            }
            SmtpSecurity::StartTLS => {
                let tls_params = lettre::transport::smtp::client::TlsParameters::new(self.account.smtp_server.clone())
                    .map_err(|e| EmailError::SmtpError(e.to_string()))?;
                
                SmtpTransport::relay(&self.account.smtp_server)
                    .map_err(|e| EmailError::SmtpError(e.to_string()))?
                    .credentials(creds)
                    .port(self.account.smtp_port)
                    .tls(lettre::transport::smtp::client::Tls::Required(tls_params))
                    .build()
            }
            SmtpSecurity::None => {
                SmtpTransport::relay(&self.account.smtp_server)
                    .map_err(|e| EmailError::SmtpError(e.to_string()))?
                    .credentials(creds)
                    .port(self.account.smtp_port)
                    .build()
            }
        };
        
        // Send the email
        mailer.send(&message)
            .map_err(|e| EmailError::SmtpError(e.to_string()))?;
        
        Ok(())
    }
    
    pub fn mark_as_read(&self, email: &Email) -> Result<(), EmailError> {
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "+FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "+FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
    
    pub fn mark_as_unread(&self, email: &Email) -> Result<(), EmailError> {
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "-FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "-FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
    
    pub fn delete_email(&self, email: &Email) -> Result<(), EmailError> {
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "+FLAGS (\\Deleted)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .expunge()
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .store(&email.id, "+FLAGS (\\Deleted)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .expunge()
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
    
    pub fn move_email(&self, email: &Email, target_folder: &str) -> Result<(), EmailError> {
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .mv(&email.id, target_folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .mv(&email.id, target_folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
}

// Background email fetcher with improved thread safety
pub struct EmailFetcher {
    client: EmailClient,
    tx: mpsc::Sender<Vec<Email>>,
    interval: std::time::Duration,
    running: Arc<Mutex<bool>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl EmailFetcher {
    pub fn new(
        client: EmailClient, 
        tx: mpsc::Sender<Vec<Email>>,
        interval_secs: u64,
    ) -> Self {
        Self {
            client,
            tx,
            interval: std::time::Duration::from_secs(interval_secs),
            running: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }
    
    pub fn start(&mut self) {
        // Set running flag
        {
            let mut running = self.running.lock().unwrap();
            *running = true;
        }
        
        let client = self.client.clone();
        let tx = self.tx.clone();
        let interval = self.interval;
        let running = self.running.clone();
        
        let handle = std::thread::spawn(move || {
            let current_folder = "INBOX".to_string();
            
            while {
                let should_continue = {
                    let running_guard = running.lock().unwrap();
                    *running_guard
                };
                should_continue
            } {
                // Fetch emails without holding the lock during network operations
                match client.fetch_emails(&current_folder, 50) {
                    Ok(emails) => {
                        // Try to send emails, but don't block if receiver is full
                        if let Err(e) = tx.try_send(emails) {
                            match e {
                                mpsc::error::TrySendError::Full(_) => {
                                    // Channel is full, skip this update
                                    eprintln!("Email channel full, skipping update");
                                }
                                mpsc::error::TrySendError::Closed(_) => {
                                    // Receiver is closed, exit the loop
                                    eprintln!("Email channel closed, stopping fetcher");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch emails: {}", e);
                    }
                }
                
                // Sleep for the specified interval
                std::thread::sleep(interval);
            }
        });
        
        self.handle = Some(handle);
    }
    
    pub fn stop(&mut self) {
        // Set running flag to false
        {
            let mut running = self.running.lock().unwrap();
            *running = false;
        }
        
        // Wait for the thread to finish
        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                eprintln!("Error joining email fetcher thread: {:?}", e);
            }
        }
    }
    
    pub fn set_folder(&self, folder: String) {
        // For now, we'll keep it simple and just use INBOX
        // In a more advanced implementation, we could use channels to communicate
        // folder changes to the background thread
        eprintln!("Folder change requested: {}", folder);
    }
}

impl Drop for EmailFetcher {
    fn drop(&mut self) {
        self.stop();
    }
}
