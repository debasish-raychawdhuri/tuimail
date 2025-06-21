use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, mpsc};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use imap::Session;
use lettre::message::{Mailbox, MultiPart, SinglePart, Attachment};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use native_tls::{TlsConnector, TlsStream};
use thiserror::Error;
use serde::{Serialize, Deserialize};

use crate::config::{EmailAccount, ImapSecurity, SmtpSecurity};
use crate::credentials::SecureCredentials;
use crate::database::EmailDatabase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderMetadata {
    pub last_uid: u32,
    pub total_messages: u32,
    pub last_sync: DateTime<Local>,
    pub downloaded_uids: HashSet<u32>,
}

impl FolderMetadata {
    fn new() -> Self {
        Self {
            last_uid: 0,
            total_messages: 0,
            last_sync: Local::now(),
            downloaded_uids: std::collections::HashSet::new(),
        }
    }
}

// Helper function to log debug information to a file
pub fn debug_log(message: &str) {
    if std::env::var("EMAIL_DEBUG").is_ok() {
        let log_file = "/tmp/tuimail_debug.log";
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] {}", timestamp, message);
        }
    }
}

// Helper function to initialize debug logging
fn init_debug_log() {
    if std::env::var("EMAIL_DEBUG").is_ok() {
        let log_file = "/tmp/tuimail_debug.log";
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_file)
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] === Email Client Debug Log Started ===", timestamp);
        }
    }
}

// Helper function to parse email addresses from header values
fn parse_email_addresses(value: &str) -> Vec<EmailAddress> {
    let mut addresses = Vec::new();
    
    debug_log(&format!("Parsing email addresses from: '{}'", value));
    
    // Handle multiple addresses separated by commas
    for addr_part in value.split(',') {
        let addr_part = addr_part.trim();
        if addr_part.is_empty() {
            continue;
        }
        
        debug_log(&format!("Processing address part: '{}'", addr_part));
        
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
                
                debug_log(&format!("Extracted: name='{}', email='{}'", clean_name, email_addr));
                
                addresses.push(EmailAddress {
                    name: if clean_name.is_empty() { None } else { Some(clean_name.to_string()) },
                    address: email_addr.to_string(),
                });
            }
        } else if addr_part.contains('@') {
            // No angle brackets, assume the whole thing is an email
            debug_log(&format!("Simple email format: '{}'", addr_part));
            addresses.push(EmailAddress {
                name: None,
                address: addr_part.to_string(),
            });
        } else {
            debug_log(&format!("Unrecognized address format: '{}'", addr_part));
        }
    }
    
    debug_log(&format!("Parsed {} addresses total", addresses.len()));
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
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
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
    
    /// Get Reply-To addresses from headers
    pub fn reply_to(&self) -> Vec<EmailAddress> {
        if let Some(reply_to_str) = self.headers.get("Reply-To") {
            // Simple parsing - in a real implementation you'd want proper email parsing
            vec![EmailAddress {
                name: None,
                address: reply_to_str.clone(),
            }]
        } else {
            Vec::new()
        }
    }
    
    /// Get Message-ID from headers
    pub fn message_id(&self) -> String {
        self.headers.get("Message-ID").cloned().unwrap_or_default()
    }
    
    /// Get References from headers
    pub fn references(&self) -> Vec<String> {
        if let Some(refs_str) = self.headers.get("References") {
            refs_str.split_whitespace().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }
    
    /// Set In-Reply-To header
    pub fn set_in_reply_to(&mut self, message_id: String) {
        self.headers.insert("In-Reply-To".to_string(), message_id);
    }
    
    /// Set References header
    pub fn set_references(&mut self, references: Vec<String>) {
        if !references.is_empty() {
            self.headers.insert("References".to_string(), references.join(" "));
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
        debug_log(&format!("Email subject: '{}'", email.subject));
        
        // Extract date
        if let Some(date) = parsed.date() {
            email.date = DateTime::from_timestamp(date.to_timestamp(), 0)
                .unwrap_or_else(|| Utc::now())
                .with_timezone(&Local);
        }
        
        debug_log("Starting header extraction...");
        
        // First, try to use mail_parser's built-in address extraction
        let from_header = parsed.from();
        match from_header {
            mail_parser::HeaderValue::Address(addr) => {
                debug_log("Found single from address using mail_parser API");
                let name = addr.name.as_ref().map(|s| s.to_string());
                let address = addr.address.as_ref().map(|s| s.to_string()).unwrap_or_default();
                debug_log(&format!("  From via API: name={:?}, address='{}'", name, address));
                
                if !address.is_empty() {
                    email.from.push(EmailAddress { name, address });
                }
            }
            mail_parser::HeaderValue::AddressList(addrs) => {
                debug_log(&format!("Found {} from addresses using mail_parser API", addrs.len()));
                for (i, addr) in addrs.iter().enumerate() {
                    let name = addr.name.as_ref().map(|s| s.to_string());
                    let address = addr.address.as_ref().map(|s| s.to_string()).unwrap_or_default();
                    debug_log(&format!("  From[{}] via API: name={:?}, address='{}'", i, name, address));
                    
                    if !address.is_empty() {
                        email.from.push(EmailAddress { name, address });
                    }
                }
            }
            _ => {
                debug_log("No from addresses found via mail_parser API");
            }
        }
        
        // Extract headers and parse addresses from them
        let mut header_count = 0;
        for header in parsed.headers() {
            header_count += 1;
            let name = header.name().to_string();
            
            // Try multiple ways to extract header value
            let value = if let Some(text_value) = header.value().as_text_ref() {
                Some(text_value.to_string())
            } else {
                // Try to get raw value and decode it
                match header.value() {
                    mail_parser::HeaderValue::Text(t) => Some(t.to_string()),
                    mail_parser::HeaderValue::TextList(list) => {
                        Some(list.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(", "))
                    }
                    mail_parser::HeaderValue::Address(addr) => {
                        // Extract address information manually
                        let name = addr.name.as_ref().map(|n| n.to_string()).unwrap_or_default();
                        let email = addr.address.as_ref().map(|s| s.to_string()).unwrap_or_default();
                        if name.is_empty() {
                            Some(email)
                        } else {
                            Some(format!("{} <{}>", name, email))
                        }
                    }
                    mail_parser::HeaderValue::AddressList(list) => {
                        let addr_strings: Vec<String> = list.iter().map(|addr| {
                            let name = addr.name.as_ref().map(|n| n.to_string()).unwrap_or_default();
                            let email = addr.address.as_ref().map(|s| s.to_string()).unwrap_or_default();
                            if name.is_empty() {
                                email
                            } else {
                                format!("{} <{}>", name, email)
                            }
                        }).collect();
                        Some(addr_strings.join(", "))
                    }
                    mail_parser::HeaderValue::DateTime(dt) => {
                        Some(format!("{}", dt))
                    }
                    _ => None
                }
            };
            
            if let Some(value_str) = value {
                email.headers.insert(name.clone(), value_str.clone());
                debug_log(&format!("Header[{}]: '{}' = '{}'", header_count, name, value_str));
                
                // Parse basic from/to information from headers
                match name.to_lowercase().as_str() {
                    "from" => {
                        debug_log(&format!("Found From header: '{}'", value_str));
                        let addresses = parse_email_addresses(&value_str);
                        debug_log(&format!("Parsed {} addresses from From header", addresses.len()));
                        email.from.extend(addresses);
                    }
                    "to" => {
                        let addresses = parse_email_addresses(&value_str);
                        email.to.extend(addresses);
                    }
                    "cc" => {
                        let addresses = parse_email_addresses(&value_str);
                        email.cc.extend(addresses);
                    }
                    _ => {}
                }
            } else {
                debug_log(&format!("Header[{}]: '{}' has no extractable value", header_count, name));
            }
        }
        
        debug_log(&format!("Processed {} headers total", header_count));
        
        // If we still don't have a from address, try to create one from the headers map
        if email.from.is_empty() {
            debug_log("No from addresses found, trying headers map fallback");
            if let Some(from_value) = email.headers.get("From").or_else(|| email.headers.get("from")) {
                debug_log(&format!("Found From in headers map: '{}'", from_value));
                let addresses = parse_email_addresses(from_value);
                email.from.extend(addresses);
            } else {
                debug_log("No From header found in headers map either");
            }
        }
        
        // Extract body parts
        if let Some(text_body) = parsed.body_text(0) {
            email.body_text = Some(text_body.to_string());
            debug_log(&format!("Extracted text body: {} chars", text_body.len()));
        }
        
        if let Some(html_body) = parsed.body_html(0) {
            email.body_html = Some(html_body.to_string());
            debug_log(&format!("Extracted HTML body: {} chars", html_body.len()));
        }
        
        // Extract attachments
        debug_log("=== STARTING ATTACHMENT EXTRACTION ===");
        email.attachments = Self::extract_attachments(parsed);
        debug_log(&format!("=== FINISHED ATTACHMENT EXTRACTION: {} attachments ===", email.attachments.len()));
        for (i, att) in email.attachments.iter().enumerate() {
            debug_log(&format!("  ATTACHMENT {}: {} ({} bytes, {})", 
                i + 1, att.filename, att.data.len(), att.content_type));
        }
        
        debug_log(&format!("Final email from addresses: {} total", email.from.len()));
        for (i, addr) in email.from.iter().enumerate() {
            debug_log(&format!("  Final From[{}]: name={:?}, address='{}'", i, addr.name, addr.address));
        }
        
        Ok(email)
    }
    
    /// Extract attachments from a parsed email message
    fn extract_attachments(parsed: &mail_parser::Message) -> Vec<EmailAttachment> {
        let mut attachments = Vec::new();
        
        debug_log(&format!("=== PARSING MESSAGE WITH {} PARTS ===", parsed.parts.len()));
        
        // Iterate through all parts of the message
        for (i, part) in parsed.parts.iter().enumerate() {
            debug_log(&format!("=== PROCESSING PART {} ({} headers) ===", i, part.headers.len()));
            
            // Debug: Print all headers for this part
            for header in &part.headers {
                let header_name = format!("{:?}", header.name);
                let header_value = match &header.value {
                    mail_parser::HeaderValue::Text(text) => text.as_ref(),
                    mail_parser::HeaderValue::TextList(list) => {
                        if let Some(first) = list.first() {
                            first.as_ref()
                        } else {
                            "empty_list"
                        }
                    }
                    _ => "other_type",
                };
                debug_log(&format!("  HEADER: {} = {}", header_name, header_value));
            }
            
            // Check if this part is an attachment
            if let Some(attachment) = Self::extract_attachment_from_part(part) {
                debug_log(&format!("=== FOUND ATTACHMENT IN PART {}: {} ===", i, attachment.filename));
                attachments.push(attachment);
            } else {
                debug_log(&format!("=== PART {} IS NOT AN ATTACHMENT ===", i));
            }
        }
        
        debug_log(&format!("=== TOTAL ATTACHMENTS FOUND: {} ===", attachments.len()));
        attachments
    }
    
    /// Extract attachment from a message part if it is an attachment
    fn extract_attachment_from_part(part: &mail_parser::MessagePart) -> Option<EmailAttachment> {
        debug_log("Checking part for attachment...");
        
        // Check if this part has a filename (indicating it's an attachment)
        let mut filename = None;
        let mut content_type = "application/octet-stream".to_string();
        let mut is_attachment = false;
        
        // Look through headers to find content-disposition and content-type
        debug_log(&format!("Processing {} headers for attachment detection", part.headers.len()));
        for (header_idx, header) in part.headers.iter().enumerate() {
            // Convert header name to string for comparison
            let header_name_str = format!("{:?}", header.name).to_lowercase();
            debug_log(&format!("Header {}: {} = {:?}", header_idx, header_name_str, header.value));
            
            // Try different ways to extract header value
            let header_value_str = match &header.value {
                mail_parser::HeaderValue::Text(text) => {
                    debug_log(&format!("Header value is Text: {}", text.as_ref()));
                    Some(text.as_ref())
                }
                mail_parser::HeaderValue::TextList(list) => {
                    if let Some(first) = list.first() {
                        debug_log(&format!("Header value is TextList: {}", first.as_ref()));
                        Some(first.as_ref())
                    } else {
                        debug_log("Header value is empty TextList");
                        None
                    }
                }
                _ => {
                    debug_log(&format!("Header value is structured type: {:?}", header.value));
                    None
                }
            };
            
            // Handle structured headers specially
            match &header.value {
                mail_parser::HeaderValue::ContentType(ct) => {
                    debug_log(&format!("Found structured ContentType: {:?}", ct));
                    // Build full content type string
                    if let Some(subtype) = ct.subtype() {
                        content_type = format!("{}/{}", ct.ctype(), subtype);
                    } else {
                        content_type = ct.ctype().to_string();
                    }
                    debug_log(&format!("Extracted content type: {}", content_type));
                    
                    // Check for name parameter in content-type
                    if let Some(name) = ct.attribute("name") {
                        filename = Some(name.to_string());
                        debug_log(&format!("Found filename in content-type: {}", name));
                    }
                    // Also check for filename parameter in content-type
                    if filename.is_none() {
                        if let Some(fname) = ct.attribute("filename") {
                            filename = Some(fname.to_string());
                            debug_log(&format!("Found filename parameter in content-type: {}", fname));
                        }
                    }
                }
                _ => {
                    // Handle text-based headers
                    if let Some(header_value) = header_value_str {
                        debug_log(&format!("Checking header: {} = {}", header_name_str, header_value));
                        
                        if header_name_str.contains("contentdisposition") || header_name_str.contains("content-disposition") {
                            debug_log("Found content-disposition header");
                            // Simple parsing for filename parameter
                            if header_value.contains("attachment") || header_value.contains("inline") {
                                is_attachment = true;
                                debug_log("Part is marked as attachment or inline");
                                
                                // Try multiple filename patterns
                                let filename_patterns = [
                                    "filename=",
                                    "filename*=",
                                    "name=",
                                ];
                                
                                for pattern in &filename_patterns {
                                    if filename.is_none() {
                                        if let Some(start) = header_value.find(pattern) {
                                            let filename_part = &header_value[start + pattern.len()..];
                                            let filename_part = filename_part.trim_start_matches('"').trim_start_matches('\'');
                                            
                                            let extracted_name = if let Some(end) = filename_part.find('"') {
                                                filename_part[..end].to_string()
                                            } else if let Some(end) = filename_part.find('\'') {
                                                filename_part[..end].to_string()
                                            } else if let Some(end) = filename_part.find(';') {
                                                filename_part[..end].trim().to_string()
                                            } else {
                                                filename_part.trim().to_string()
                                            };
                                            
                                            if !extracted_name.is_empty() {
                                                filename = Some(extracted_name);
                                                debug_log(&format!("Extracted filename using pattern '{}': {:?}", pattern, filename));
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        } else if header_name_str.contains("contenttype") || header_name_str.contains("content-type") {
                            debug_log("Found content-type header");
                            if let Some(semicolon_pos) = header_value.find(';') {
                                content_type = header_value[..semicolon_pos].trim().to_string();
                            } else {
                                content_type = header_value.trim().to_string();
                            }
                            debug_log(&format!("Content type: {}", content_type));
                            
                            // Also check for name parameter in content-type
                            if filename.is_none() {
                                let name_patterns = ["name=", "filename="];
                                
                                for pattern in &name_patterns {
                                    if let Some(start) = header_value.find(pattern) {
                                        let name_part = &header_value[start + pattern.len()..];
                                        let name_part = name_part.trim_start_matches('"').trim_start_matches('\'');
                                        
                                        let extracted_name = if let Some(end) = name_part.find('"') {
                                            name_part[..end].to_string()
                                        } else if let Some(end) = name_part.find('\'') {
                                            name_part[..end].to_string()
                                        } else if let Some(end) = name_part.find(';') {
                                            name_part[..end].trim().to_string()
                                        } else {
                                            name_part.trim().to_string()
                                        };
                                        
                                        if !extracted_name.is_empty() {
                                            filename = Some(extracted_name);
                                            debug_log(&format!("Extracted filename from content-type using pattern '{}': {:?}", pattern, filename));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If we have a filename or it's marked as attachment, try to extract it
        // Be conservative: require explicit attachment markers or non-text content
        let is_likely_attachment = is_attachment || 
            (filename.is_some() && !content_type.starts_with("text/plain")) || 
            (!content_type.starts_with("text/") && 
             !content_type.starts_with("multipart/") && 
             content_type != "application/octet-stream");
        
        if is_likely_attachment {
            debug_log(&format!("Final attachment analysis: content_type={}, filename={:?}, is_attachment={}", 
                content_type, filename, is_attachment));
            
            let final_filename = filename.unwrap_or_else(|| {
                debug_log("WARNING: No filename found in email headers - this should not happen for proper attachments");
                // Only use simple fallback - the real fix is to extract the filename properly
                match content_type.as_str() {
                    "application/pdf" => "document.pdf".to_string(),
                    "image/jpeg" => "image.jpg".to_string(),
                    "image/png" => "image.png".to_string(),
                    "image/gif" => "image.gif".to_string(),
                    "application/zip" => "archive.zip".to_string(),
                    "application/msword" => "document.doc".to_string(),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "document.docx".to_string(),
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "spreadsheet.xlsx".to_string(),
                    "application/vnd.ms-excel" => "spreadsheet.xls".to_string(),
                    "text/plain" => "text.txt".to_string(),
                    "text/csv" => "data.csv".to_string(),
                    _ => {
                        let extension = content_type.split('/').last().unwrap_or("bin");
                        format!("attachment.{}", extension)
                    }
                }
            });
            
            debug_log(&format!("Treating as attachment: content_type={}, filename={}, is_attachment={}", 
                content_type, final_filename, is_attachment));
            
            // Get the body data
            let data = match &part.body {
                mail_parser::PartType::Text(text) => {
                    debug_log("Part body is text");
                    text.as_bytes().to_vec()
                }
                mail_parser::PartType::Html(html) => {
                    debug_log("Part body is HTML");
                    html.as_bytes().to_vec()
                }
                mail_parser::PartType::Binary(binary) => {
                    debug_log("Part body is binary");
                    binary.to_vec()
                }
                mail_parser::PartType::InlineBinary(binary) => {
                    debug_log("Part body is inline binary");
                    binary.to_vec()
                }
                mail_parser::PartType::Message(_) => {
                    debug_log("Part body is nested message - skipping");
                    Vec::new()
                }
                mail_parser::PartType::Multipart(_) => {
                    debug_log("Part body is multipart container - skipping");
                    Vec::new()
                }
            };
            
            debug_log(&format!("Extracted {} bytes of data", data.len()));
            
            if !data.is_empty() {
                debug_log(&format!("Creating attachment: {} ({} bytes, {})", 
                    final_filename, data.len(), content_type));
                
                return Some(EmailAttachment {
                    filename: final_filename,
                    content_type,
                    data,
                });
            } else {
                debug_log("No data found in part body");
            }
        } else {
            debug_log(&format!("Part not treated as attachment: content_type={}, filename={:?}, is_attachment={}", 
                content_type, filename, is_attachment));
        }
        
        None
    }
}

#[derive(Clone)]
pub struct EmailClient {
    account: EmailAccount,
    credentials: SecureCredentials,
    db_path: std::path::PathBuf,
}

impl EmailClient {
    pub fn new(account: EmailAccount, credentials: SecureCredentials) -> Self {
        init_debug_log();
        debug_log(&format!("Creating EmailClient for account: {}", account.email));
        
        let cache_dir = format!("{}/.cache/tuimail/{}", 
            dirs::home_dir().unwrap_or_default().display(), 
            account.email.replace('@', "_at_").replace('.', "_"));
        
        // Create cache directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            debug_log(&format!("Warning: Could not create cache directory {}: {}", cache_dir, e));
        }

        // Set up database path
        let db_path = std::path::PathBuf::from(&cache_dir).join("emails.db");
        
        Self { account, credentials, db_path }
    }
    
    fn get_database(&self) -> Result<EmailDatabase, EmailError> {
        EmailDatabase::new(&self.db_path)
            .map_err(|e| EmailError::ConnectionError(format!("Database error: {}", e)))
    }
    
    fn load_folder_metadata(&self, folder: &str) -> FolderMetadata {
        match self.get_database() {
            Ok(db) => {
                match db.load_folder_metadata(&self.account.email, folder) {
                    Ok((last_uid, total_messages, _last_sync)) => {
                        debug_log(&format!("Loaded metadata from database: last_uid={}, total_messages={}", last_uid, total_messages));
                        FolderMetadata {
                            last_uid,
                            total_messages,
                            last_sync: Local::now(),
                            downloaded_uids: std::collections::HashSet::new(),
                        }
                    }
                    Err(e) => {
                        debug_log(&format!("Failed to load metadata from database: {}", e));
                        FolderMetadata::new()
                    }
                }
            }
            Err(e) => {
                debug_log(&format!("Failed to open database for metadata: {}", e));
                FolderMetadata::new()
            }
        }
    }
    
    fn save_folder_metadata(&self, folder: &str, metadata: &FolderMetadata) {
        match self.get_database() {
            Ok(db) => {
                if let Err(e) = db.save_folder_metadata(&self.account.email, folder, metadata.last_uid, metadata.total_messages) {
                    debug_log(&format!("Warning: Could not save folder metadata to database: {}", e));
                } else {
                    debug_log(&format!("Saved metadata to database: last_uid={}, total_messages={}", metadata.last_uid, metadata.total_messages));
                }
            }
            Err(e) => {
                debug_log(&format!("Failed to open database for saving metadata: {}", e));
            }
        }
    }
    
    fn load_cached_emails(&self, folder: &str) -> Vec<Email> {
        match self.get_database() {
            Ok(db) => {
                match db.load_emails(&self.account.email, folder) {
                    Ok(emails) => {
                        debug_log(&format!("Loaded {} emails from database for folder: {}", emails.len(), folder));
                        emails
                    }
                    Err(e) => {
                        debug_log(&format!("Failed to load emails from database: {}", e));
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                debug_log(&format!("Failed to open database: {}", e));
                Vec::new()
            }
        }
    }
    
    fn save_cached_emails(&self, folder: &str, emails: &[Email]) {
        match self.get_database() {
            Ok(db) => {
                if let Err(e) = db.save_emails(&self.account.email, folder, emails) {
                    log::warn!("Could not save emails to database: {}", e);
                    debug_log(&format!("Database save error: {}", e));
                } else {
                    debug_log(&format!("Saved {} emails to database for folder: {}", emails.len(), folder));
                }
            }
            Err(e) => {
                log::warn!("Could not open database for saving: {}", e);
                debug_log(&format!("Database open error: {}", e));
            }
        }
    }
    
    #[allow(dead_code)]
    pub fn force_full_sync(&self, folder: &str) -> Result<Vec<Email>, EmailError> {
        debug_log(&format!("force_full_sync called for folder: {}", folder));
        
        // Reset metadata to force full sync
        let mut metadata = FolderMetadata::new();
        
        let new_emails = match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                self.fetch_emails_incrementally_secure(folder, &mut metadata)
            }
            ImapSecurity::None => {
                self.fetch_emails_incrementally_plain(folder, &mut metadata)
            }
        };
        
        match new_emails {
            Ok(emails) => {
                // Save the emails and metadata
                self.save_cached_emails(folder, &emails);
                self.save_folder_metadata(folder, &metadata);
                debug_log(&format!("Full sync completed: {} emails", emails.len()));
                Ok(emails)
            }
            Err(e) => {
                debug_log(&format!("Full sync failed: {}", e));
                Err(e)
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
        let password = self.account.get_imap_password(&self.credentials)
            .map_err(|e| EmailError::ImapError(format!("Failed to get IMAP password: {}", e)))?;
        
        let tls = TlsConnector::builder().build()?;
        let client = imap::connect((domain.as_str(), port), domain, &tls)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let session = client
            .login(username, &password)
            .map_err(|e| EmailError::ImapError(e.0.to_string()))?;
        
        Ok(session)
    }
    
    fn connect_imap_plain(&self) -> Result<Session<std::net::TcpStream>, EmailError> {
        let domain = &self.account.imap_server;
        let port = self.account.imap_port;
        let username = &self.account.imap_username;
        let password = self.account.get_imap_password(&self.credentials)
            .map_err(|e| EmailError::ImapError(format!("Failed to get IMAP password: {}", e)))?;
        
        let tcp_stream = std::net::TcpStream::connect((domain.as_str(), port))
            .map_err(|e| EmailError::IoError(e))?;
        
        let client = imap::Client::new(tcp_stream);
        let session = client
            .login(username, &password)
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
        debug_log(&format!("fetch_emails called: folder='{}', limit={}", folder, limit));
        
        // Load cached emails and metadata
        let cached_emails = self.load_cached_emails(folder);
        let mut metadata = self.load_folder_metadata(folder);
        debug_log(&format!("Loaded {} cached emails, last_uid={}, total_messages={}", 
            cached_emails.len(), metadata.last_uid, metadata.total_messages));
        
        // Fetch new emails from server incrementally
        debug_log(&format!("Fetching new emails from server using security: {:?}", self.account.imap_security));
        let new_emails = match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                self.fetch_emails_incrementally_secure(folder, &mut metadata)
            }
            ImapSecurity::None => {
                self.fetch_emails_incrementally_plain(folder, &mut metadata)
            }
        };
        
        match new_emails {
            Ok(new) => {
                debug_log(&format!("Successfully fetched {} new emails from server", new.len()));
                
                // Merge cached and new emails
                let merged = self.merge_emails(cached_emails, new);
                debug_log(&format!("After merging: {} total emails", merged.len()));
                
                // Update metadata
                metadata.last_sync = Local::now();
                metadata.total_messages = merged.len() as u32;
                
                // Save updated cache and metadata
                self.save_cached_emails(folder, &merged);
                self.save_folder_metadata(folder, &metadata);
                debug_log("Saved updated cache and metadata");
                
                // Return all emails (or limited for display)
                let display_limit = if limit == 0 { merged.len() } else { std::cmp::max(limit, 100) }; // Show all if limit is 0
                let result_count = std::cmp::min(display_limit, merged.len());
                debug_log(&format!("Returning {} emails for display (limit={}, merged={})", result_count, limit, merged.len()));
                Ok(merged.into_iter().take(result_count).collect())
            }
            Err(e) => {
                debug_log(&format!("Server fetch failed: {}", e));
                // If server fetch fails, return cached emails
                if !cached_emails.is_empty() {
                    debug_log(&format!("Using {} cached emails due to server error", cached_emails.len()));
                    Ok(cached_emails)
                } else {
                    debug_log("No cached emails available, returning error");
                    Err(e)
                }
            }
        }
    }
    
    fn fetch_emails_incrementally_secure(&self, folder: &str, metadata: &mut FolderMetadata) -> Result<Vec<Email>, EmailError> {
        let tls = TlsConnector::builder().build().unwrap();
        let client = imap::connect(
            (self.account.imap_server.as_str(), self.account.imap_port),
            &self.account.imap_server,
            &tls,
        ).map_err(|e| EmailError::ImapError(e.to_string()))?;

        let password = self.account.get_imap_password(&self.credentials)
            .map_err(|e| EmailError::ImapError(format!("Failed to get IMAP password: {}", e)))?;

        let mut session = client
            .login(&self.account.imap_username, &password)
            .map_err(|e| EmailError::ImapError(e.0.to_string()))?;

        session
            .select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;

        // Get current folder status
        let mailbox = session.examine(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let current_total = mailbox.exists;
        debug_log(&format!("Folder '{}' has {} total messages, we have {} cached", 
            folder, current_total, metadata.downloaded_uids.len()));

        // First time sync - fetch ALL messages
        if metadata.last_uid == 0 {
            debug_log("First time sync - fetching ALL messages");
            
            // Check if the folder is empty
            if current_total == 0 {
                debug_log("Folder is empty, skipping fetch");
                return Ok(Vec::new()); // Return empty vector for empty folders
            }
            
            // For initial sync, fetch ALL messages in batches to avoid memory issues
            let batch_size = 500; // Fetch in batches of 500
            let mut all_emails = Vec::new();
            let mut current_seq = 1;
            
            while current_seq <= current_total {
                let end_seq = std::cmp::min(current_seq + batch_size - 1, current_total);
                let sequence = format!("{}:{}", current_seq, end_seq);
                
                debug_log(&format!("Initial sync batch: fetching messages {} (batch {}/{})", 
                    sequence, (current_seq - 1) / batch_size + 1, (current_total + batch_size - 1) / batch_size));
                
                let messages = session
                    .fetch(&sequence, "(RFC822 FLAGS UID)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;

                debug_log(&format!("Fetched {} messages in this batch", messages.len()));
                
                let batch_emails = self.parse_messages(&messages, folder)?;
                all_emails.extend(batch_emails);
                
                // Update metadata with all fetched UIDs
                for message in &messages {
                    if let Some(uid) = message.uid {
                        metadata.downloaded_uids.insert(uid);
                        if uid > metadata.last_uid {
                            metadata.last_uid = uid;
                        }
                    }
                }
                
                current_seq = end_seq + 1;
                
                // Small delay between batches to be nice to the server
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            
            metadata.total_messages = current_total;
            debug_log(&format!("Initial sync complete: fetched {} total emails", all_emails.len()));
            
            return Ok(all_emails);
        }

        // Incremental sync - fetch only new messages
        if current_total <= metadata.total_messages {
            debug_log("No new messages to fetch");
            return Ok(Vec::new());
        }

        let start_uid = metadata.last_uid + 1;
        debug_log(&format!("Incremental sync: fetching messages with UID >= {}", start_uid));

        // Use UID FETCH to get only new messages
        let sequence = format!("{}:*", start_uid);
        let messages = session
            .uid_fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;

        debug_log(&format!("Incremental sync: fetched {} new messages", messages.len()));

        let new_emails = self.parse_messages(&messages, folder)?;
        
        // Update metadata with new UIDs
        for message in &messages {
            if let Some(uid) = message.uid {
                metadata.downloaded_uids.insert(uid);
                if uid > metadata.last_uid {
                    metadata.last_uid = uid;
                }
            }
        }
        metadata.total_messages = current_total;

        Ok(new_emails)
    }

    fn fetch_emails_incrementally_plain(&self, folder: &str, metadata: &mut FolderMetadata) -> Result<Vec<Email>, EmailError> {
        let mut session = self.connect_imap_plain()?;
        
        session
            .select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;

        // Get current folder status
        let mailbox = session.examine(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let current_total = mailbox.exists;
        debug_log(&format!("Folder '{}' has {} total messages, we have {} cached", 
            folder, current_total, metadata.downloaded_uids.len()));

        // First time sync - fetch ALL messages
        if metadata.last_uid == 0 {
            debug_log("First time sync - fetching ALL messages");
            
            // Check if the folder is empty
            if current_total == 0 {
                debug_log("Folder is empty, skipping fetch");
                return Ok(Vec::new()); // Return empty vector for empty folders
            }
            
            // For initial sync, fetch ALL messages in batches to avoid memory issues
            let batch_size = 500; // Fetch in batches of 500
            let mut all_emails = Vec::new();
            let mut current_seq = 1;
            
            while current_seq <= current_total {
                let end_seq = std::cmp::min(current_seq + batch_size - 1, current_total);
                let sequence = format!("{}:{}", current_seq, end_seq);
                
                debug_log(&format!("Initial sync batch: fetching messages {} (batch {}/{})", 
                    sequence, (current_seq - 1) / batch_size + 1, (current_total + batch_size - 1) / batch_size));
                
                let messages = session
                    .fetch(&sequence, "(RFC822 FLAGS UID)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;

                debug_log(&format!("Fetched {} messages in this batch", messages.len()));
                
                let batch_emails = self.parse_messages(&messages, folder)?;
                all_emails.extend(batch_emails);
                
                // Update metadata with all fetched UIDs
                for message in &messages {
                    if let Some(uid) = message.uid {
                        metadata.downloaded_uids.insert(uid);
                        if uid > metadata.last_uid {
                            metadata.last_uid = uid;
                        }
                    }
                }
                
                current_seq = end_seq + 1;
                
                // Small delay between batches to be nice to the server
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            
            metadata.total_messages = current_total;
            debug_log(&format!("Initial sync complete: fetched {} total emails", all_emails.len()));
            
            return Ok(all_emails);
        }

        // Incremental sync - fetch only new messages
        if current_total <= metadata.total_messages {
            debug_log("No new messages to fetch");
            return Ok(Vec::new());
        }

        let start_uid = metadata.last_uid + 1;
        debug_log(&format!("Incremental sync: fetching messages with UID >= {}", start_uid));

        // Use UID FETCH to get only new messages
        let sequence = format!("{}:*", start_uid);
        let messages = session
            .uid_fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;

        debug_log(&format!("Incremental sync: fetched {} new messages", messages.len()));

        let new_emails = self.parse_messages(&messages, folder)?;
        
        // Update metadata with new UIDs
        for message in &messages {
            if let Some(uid) = message.uid {
                metadata.downloaded_uids.insert(uid);
                if uid > metadata.last_uid {
                    metadata.last_uid = uid;
                }
            }
        }
        metadata.total_messages = current_total;

        Ok(new_emails)
    }
    
    fn parse_messages(&self, messages: &[imap::types::Fetch], folder: &str) -> Result<Vec<Email>, EmailError> {
        let mut emails = Vec::new();
        
        debug_log(&format!("Starting to parse {} messages from folder '{}'", messages.len(), folder));
        
        for (i, message) in messages.iter().enumerate() {
            if let Some(body) = message.body() {
                // Skip messages without valid UIDs
                let uid = match message.uid {
                    Some(uid) if uid > 0 => uid.to_string(),
                    _ => {
                        debug_log(&format!("Message {} has invalid UID ({:?}), skipping", i + 1, message.uid));
                        continue;
                    }
                };
                
                let flags: Vec<String> = message
                    .flags()
                    .iter()
                    .map(|f| f.to_string()) // Use consistent flag parsing
                    .collect();
                
                debug_log(&format!("Message {}: UID={}, body_length={}, flags={:?}", 
                    i + 1, uid, body.len(), flags));
                
                if body.len() > 100 {
                    let preview = String::from_utf8_lossy(&body[..200.min(body.len())]);
                    debug_log(&format!("Message {} body preview: {}", i + 1, preview));
                }
                
                match mail_parser::Message::parse(body) {
                    Some(parsed) => {
                        debug_log(&format!("Message {} parsed successfully by mail_parser", i + 1));
                        match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                            Ok(mut email) => {
                                debug_log(&format!("Email parsed: subject='{}', from_count={}", 
                                    email.subject, email.from.len()));
                                
                                for (j, addr) in email.from.iter().enumerate() {
                                    debug_log(&format!("  From[{}]: name={:?}, address='{}'", 
                                        j, addr.name, addr.address));
                                }
                                
                                // Fallback: if we still don't have from addresses, try to extract from raw headers
                                if email.from.is_empty() {
                                    debug_log("No from addresses found, trying header fallback");
                                    if let Some(from_header) = email.headers.get("From").or_else(|| email.headers.get("from")) {
                                        debug_log(&format!("Found From header in fallback: '{}'", from_header));
                                        let addresses = parse_email_addresses(from_header);
                                        debug_log(&format!("Fallback parsed {} addresses", addresses.len()));
                                        email.from.extend(addresses);
                                    } else {
                                        debug_log("No From header found in fallback either");
                                        debug_log(&format!("Available headers: {:?}", email.headers.keys().collect::<Vec<_>>()));
                                    }
                                }
                                
                                emails.push(email);
                            }
                            Err(e) => {
                                debug_log(&format!("Error parsing email {}: {}", i + 1, e));
                            }
                        }
                    }
                    None => {
                        debug_log(&format!("Message {} failed to parse with mail_parser", i + 1));
                    }
                }
            } else {
                debug_log(&format!("Message {} has no body", i + 1));
            }
        }
        
        // Sort by date, newest first
        emails.sort_by(|a, b| b.date.cmp(&a.date));
        
        debug_log(&format!("Finished parsing, returning {} emails", emails.len()));
        Ok(emails)
    }
    
    pub fn send_email(&self, email: &Email) -> Result<(), EmailError> {
        // Debug: Log attachment info
        if !email.attachments.is_empty() {
            debug_log(&format!("DEBUG: Sending email with {} attachments:", email.attachments.len()));
            for (i, attachment) in email.attachments.iter().enumerate() {
                debug_log(&format!("  {}: {} ({} bytes, {})", 
                    i + 1, 
                    attachment.filename, 
                    attachment.data.len(), 
                    attachment.content_type
                ));
            }
        }
        
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
        
        // Build the email body with attachments
        let body_part = MultiPart::alternative()
            .singlepart(
                SinglePart::plain(email.body_text.clone().unwrap_or_default())
            );
        
        let final_multipart = if email.attachments.is_empty() {
            // No attachments, just use the body
            body_part
        } else {
            // Has attachments, create mixed multipart
            let mut mixed_part = MultiPart::mixed()
                .multipart(body_part);
            
            // Add attachments
            for attachment in &email.attachments {
                let attachment_part = Attachment::new(attachment.filename.clone())
                    .body(attachment.data.clone(), attachment.content_type.parse().unwrap_or("application/octet-stream".parse().unwrap()));
                mixed_part = mixed_part.singlepart(attachment_part);
            }
            
            mixed_part
        };
        
        // Build the final message
        let message = message_builder
            .multipart(final_multipart)
            .map_err(|e| EmailError::SmtpError(e.to_string()))?;
        
        // Configure SMTP transport
        let smtp_password = self.account.get_smtp_password(&self.credentials)
            .map_err(|e| EmailError::SmtpError(format!("Failed to get SMTP password: {}", e)))?;
            
        let creds = Credentials::new(
            self.account.smtp_username.clone(),
            smtp_password,
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
        debug_log(&format!("Marking email as read: {} in folder {}", email.id, email.folder));
        
        // Validate email ID before attempting STORE operation
        if email.id.is_empty() || email.id == "0" {
            debug_log(&format!("Invalid email ID '{}', skipping mark as read", email.id));
            return Err(EmailError::ImapError("Invalid email ID for STORE operation".to_string()));
        }
        
        // Add retry logic for IMAP connection issues
        let mut attempts = 0;
        let max_attempts = 3;
        
        while attempts < max_attempts {
            attempts += 1;
            
            let result = match self.account.imap_security {
                ImapSecurity::SSL | ImapSecurity::StartTLS => {
                    match self.connect_imap_secure() {
                        Ok(mut session) => {
                            match session.select(&email.folder) {
                                Ok(_) => {
                                    debug_log(&format!("Attempting STORE command with UID: {}", email.id));
                                    session.uid_store(&email.id, "+FLAGS (\\Seen)")
                                        .map_err(|e| EmailError::ImapError(e.to_string()))
                                }
                                Err(e) => Err(EmailError::ImapError(e.to_string()))
                            }
                        }
                        Err(e) => Err(e)
                    }
                }
                ImapSecurity::None => {
                    match self.connect_imap_plain() {
                        Ok(mut session) => {
                            match session.select(&email.folder) {
                                Ok(_) => {
                                    debug_log(&format!("Attempting STORE command with UID: {}", email.id));
                                    session.uid_store(&email.id, "+FLAGS (\\Seen)")
                                        .map_err(|e| EmailError::ImapError(e.to_string()))
                                }
                                Err(e) => Err(EmailError::ImapError(e.to_string()))
                            }
                        }
                        Err(e) => Err(e)
                    }
                }
            };
            
            match result {
                Ok(_) => {
                    debug_log(&format!("Successfully marked email {} as read", email.id));
                    return Ok(());
                }
                Err(e) => {
                    debug_log(&format!("Attempt {} failed to mark email as read: {}", attempts, e));
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    // Wait a bit before retrying
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
        
        Err(EmailError::ImapError("Failed to mark email as read after retries".to_string()))
    }
    
    #[allow(dead_code)]
    pub fn mark_as_unread(&self, email: &Email) -> Result<(), EmailError> {
        // Validate email ID before attempting STORE operation
        if email.id.is_empty() || email.id == "0" {
            debug_log(&format!("Invalid email ID '{}', skipping mark as unread", email.id));
            return Err(EmailError::ImapError("Invalid email ID for STORE operation".to_string()));
        }
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .uid_store(&email.id, "-FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .uid_store(&email.id, "-FLAGS (\\Seen)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
    
    pub fn delete_email(&self, email: &Email) -> Result<(), EmailError> {
        // Validate email ID before attempting STORE operation
        if email.id.is_empty() || email.id == "0" {
            debug_log(&format!("Invalid email ID '{}', skipping delete", email.id));
            return Err(EmailError::ImapError("Invalid email ID for STORE operation".to_string()));
        }
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session
                    .select(&email.folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .uid_store(&email.id, "+FLAGS (\\Deleted)")
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
                    .uid_store(&email.id, "+FLAGS (\\Deleted)")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                session
                    .expunge()
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                
                Ok(())
            }
        }
    }
    
    /// Fetch only new emails since the last known count
    fn fetch_new_emails_since_count(&self, folder: &str, last_count: usize) -> Result<Vec<Email>, EmailError> {
        debug_log(&format!("Fetching new emails since count: {}", last_count));
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                self.fetch_new_emails_since_count_secure(folder, last_count)
            }
            ImapSecurity::None => {
                self.fetch_new_emails_since_count_plain(folder, last_count)
            }
        }
    }
    
    fn fetch_new_emails_since_count_secure(&self, folder: &str, last_count: usize) -> Result<Vec<Email>, EmailError> {
        let mut session = self.connect_imap_secure()?;
        session.select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        // Get current message count
        let current_count = session.search("ALL")
            .map_err(|e| EmailError::ImapError(e.to_string()))?
            .len();
        
        if current_count <= last_count {
            debug_log("No new emails to fetch");
            return Ok(Vec::new());
        }
        
        let new_message_count = current_count - last_count;
        debug_log(&format!("Fetching {} new emails (messages {}-{})", 
            new_message_count, last_count + 1, current_count));
        
        // Fetch only the new messages
        let sequence = format!("{}:{}", last_count + 1, current_count);
        let messages = session
            .fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let mut emails = Vec::new();
        for (i, message) in messages.iter().enumerate() {
            if let Some(body) = message.body() {
                let flags: Vec<String> = message.flags().iter().map(|f| f.to_string()).collect();
                // Skip messages without valid UIDs
                let uid = match message.uid {
                    Some(uid) if uid > 0 => uid.to_string(),
                    _ => {
                        debug_log(&format!("Message {} has invalid UID ({:?}), skipping", i + 1, message.uid));
                        continue;
                    }
                };
                
                debug_log(&format!("Processing new email {}: UID={}, size={} bytes, flags={:?}", 
                    i + 1, uid, body.len(), flags));
                
                match mail_parser::Message::parse(body) {
                    Some(parsed) => {
                        match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                            Ok(email) => {
                                emails.push(email);
                            }
                            Err(e) => {
                                debug_log(&format!("Failed to create email object: {}", e));
                            }
                        }
                    }
                    None => {
                        debug_log("Failed to parse email");
                    }
                }
            }
        }
        
        debug_log(&format!("Successfully fetched {} new emails", emails.len()));
        Ok(emails)
    }
    
    fn fetch_new_emails_since_count_plain(&self, folder: &str, last_count: usize) -> Result<Vec<Email>, EmailError> {
        let mut session = self.connect_imap_plain()?;
        session.select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        // Get current message count
        let current_count = session.search("ALL")
            .map_err(|e| EmailError::ImapError(e.to_string()))?
            .len();
        
        if current_count <= last_count {
            debug_log("No new emails to fetch");
            return Ok(Vec::new());
        }
        
        let new_message_count = current_count - last_count;
        debug_log(&format!("Fetching {} new emails (messages {}-{})", 
            new_message_count, last_count + 1, current_count));
        
        // Fetch only the new messages
        let sequence = format!("{}:{}", last_count + 1, current_count);
        let messages = session
            .fetch(sequence, "(RFC822 FLAGS UID)")
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        let mut emails = Vec::new();
        for (i, message) in messages.iter().enumerate() {
            if let Some(body) = message.body() {
                let flags: Vec<String> = message.flags().iter().map(|f| f.to_string()).collect();
                // Skip messages without valid UIDs
                let uid = match message.uid {
                    Some(uid) if uid > 0 => uid.to_string(),
                    _ => {
                        debug_log(&format!("Message {} has invalid UID ({:?}), skipping", i + 1, message.uid));
                        continue;
                    }
                };
                
                debug_log(&format!("Processing new email {}: UID={}, size={} bytes, flags={:?}", 
                    i + 1, uid, body.len(), flags));
                
                match mail_parser::Message::parse(body) {
                    Some(parsed) => {
                        match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                            Ok(email) => {
                                emails.push(email);
                            }
                            Err(e) => {
                                debug_log(&format!("Failed to create email object: {}", e));
                            }
                        }
                    }
                    None => {
                        debug_log("Failed to parse email");
                    }
                }
            }
        }
        
        debug_log(&format!("Successfully fetched {} new emails", emails.len()));
        Ok(emails)
    }
    
    /// Check if the IMAP connection is still healthy
    fn is_connection_healthy_secure(&self, session: &mut imap::Session<native_tls::TlsStream<std::net::TcpStream>>) -> bool {
        // Try a lightweight NOOP command to test connection
        match session.noop() {
            Ok(_) => {
                debug_log("Connection health check: NOOP successful");
                true
            }
            Err(e) => {
                debug_log(&format!("Connection health check: NOOP failed: {}", e));
                false
            }
        }
    }
    
    /// Check if the plain IMAP connection is still healthy
    fn is_connection_healthy_plain(&self, session: &mut imap::Session<std::net::TcpStream>) -> bool {
        // Try a lightweight NOOP command to test connection
        match session.noop() {
            Ok(_) => {
                debug_log("Connection health check (plain): NOOP successful");
                true
            }
            Err(e) => {
                debug_log(&format!("Connection health check (plain): NOOP failed: {}", e));
                false
            }
        }
    }
    
    /// Sync emails after reconnection to catch any missed during disconnection
    fn sync_emails_after_reconnection(&self, folder: &str, last_known_count: usize, tx: &mpsc::Sender<Vec<Email>>) -> Result<usize, EmailError> {
        debug_log(&format!("Syncing emails after reconnection - last known count: {}", last_known_count));
        
        // Get current email count
        let current_count = match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                session.search("ALL")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?
                    .len()
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(e.to_string()))?;
                session.search("ALL")
                    .map_err(|e| EmailError::ImapError(e.to_string()))?
                    .len()
            }
        };
        
        debug_log(&format!("Reconnection sync: current count {}, last known {}", current_count, last_known_count));
        
        if current_count > last_known_count {
            let missed_count = current_count - last_known_count;
            debug_log(&format!("Found {} emails that arrived during disconnection", missed_count));
            
            // Fetch the missed emails
            match self.fetch_new_emails_since_count(folder, last_known_count) {
                Ok(missed_emails) => {
                    if !missed_emails.is_empty() {
                        debug_log(&format!("Found {} missed emails", missed_emails.len()));
                        // TODO: In new architecture, this would be handled by sync daemon
                        // if let Err(e) = database.save_emails(&self.account.email, folder, &missed_emails) {
                        //     debug_log(&format!("Failed to save missed emails to database: {}", e));
                        // } else {
                        //     debug_log("Successfully saved missed emails to database");
                        // }
                    }
                }
                Err(e) => {
                    debug_log(&format!("Failed to fetch missed emails: {}", e));
                }
            }
        } else if current_count < last_known_count {
            debug_log("Email count decreased - some emails may have been deleted");
        } else {
            debug_log("No new emails during disconnection");
        }
        
        Ok(current_count)
    }
    
    pub fn supports_idle(&self) -> bool {
        // Try to connect and check capabilities
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                if let Ok(mut session) = self.connect_imap_secure() {
                    session.capabilities().map(|caps| caps.has_str("IDLE")).unwrap_or(false)
                } else {
                    false
                }
            }
            ImapSecurity::None => {
                if let Ok(mut session) = self.connect_imap_plain() {
                    session.capabilities().map(|caps| caps.has_str("IDLE")).unwrap_or(false)
                } else {
                    false
                }
            }
        }
    }
    
    pub fn run_idle_session(
        &self,
        folder: &str,
        database: &crate::database::EmailDatabase,
        running: &Arc<Mutex<bool>>,
    ) -> Result<(), EmailError> {
        debug_log(&format!("Starting IDLE session for folder: {}", folder));
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                self.run_idle_session_secure(folder, database, running)
            }
            ImapSecurity::None => {
                self.run_idle_session_plain(folder, database, running)
            }
        }
    }
    
    fn run_idle_session_secure(
        &self,
        folder: &str,
        database: &crate::database::EmailDatabase,
        running: &Arc<Mutex<bool>>,
    ) -> Result<(), EmailError> {
        let mut reconnect_attempts = 0;
        const MAX_RECONNECT_ATTEMPTS: u32 = 10; // Increased for suspend/resume scenarios
        let mut last_known_count = 0; // Track message count across reconnections
        
        loop {
            // Check if we should stop
            {
                let running_guard = running.lock().unwrap();
                if !*running_guard {
                    debug_log("IDLE session: stopping due to running flag");
                    return Ok(());
                }
            }
            
            match self.run_single_idle_session_secure_with_count(folder, database, running, &mut last_known_count) {
                Ok(_) => {
                    debug_log("IDLE session completed normally");
                    return Ok(());
                }
                Err(e) => {
                    reconnect_attempts += 1;
                    debug_log(&format!("IDLE session error (attempt {}): {}", reconnect_attempts, e));
                    
                    if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                        debug_log("Max reconnection attempts reached, stopping IDLE session");
                        return Err(e);
                    }
                    
                    // Progressive backoff for reconnection attempts
                    let backoff_seconds = std::cmp::min(5 * reconnect_attempts, 60);
                    debug_log(&format!("Waiting {} seconds before reconnection attempt...", backoff_seconds));
                    std::thread::sleep(std::time::Duration::from_secs(backoff_seconds as u64));
                    
                    debug_log(&format!("Attempting to reconnect IDLE session (last known count: {})...", last_known_count));
                }
            }
        }
    }
    
    fn run_single_idle_session_secure_with_count(
        &self,
        folder: &str,
        database: &crate::database::EmailDatabase,
        running: &Arc<Mutex<bool>>,
        last_known_count: &mut usize,
    ) -> Result<(), EmailError> {
        let mut session = self.connect_imap_secure()?;
        session.select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        debug_log("IDLE session: connected and folder selected");
        
        // Check if server supports IDLE
        let caps = session.capabilities()
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        if !caps.has_str("IDLE") {
            debug_log("Server does not support IDLE, falling back to polling");
            return Err(EmailError::ImapError("Server does not support IDLE".to_string()));
        }
        
        debug_log("IDLE session: Server supports IDLE, starting suspend/resume resilient IDLE loop");
        
        // Sync any missed emails from previous disconnection and update count
        // TODO: In new architecture, this is handled by sync daemon
        // *last_known_count = self.sync_emails_after_reconnection(folder, *last_known_count, tx)?;
        debug_log(&format!("IDLE session: message count after reconnection sync: {}", last_known_count));
        
        // Main IDLE loop with shorter timeouts for better suspend/resume handling
        let mut consecutive_health_checks = 0;
        const MAX_HEALTH_CHECK_FAILURES: u32 = 3;
        
        loop {
            // Check if we should stop
            {
                let running_guard = running.lock().unwrap();
                if !*running_guard {
                    debug_log("IDLE session: stopping due to running flag");
                    return Ok(());
                }
            }
            
            // Use shorter IDLE timeout (30 seconds) for better suspend/resume detection
            debug_log("IDLE session: starting IDLE command with 30-second timeout");
            
            // Separate the IDLE operation to ensure proper scoping
            let idle_result = {
                match session.idle() {
                    Ok(idle_handle) => {
                        debug_log("IDLE session: IDLE started, waiting for notifications or timeout");
                        
                        // Wait for 30 seconds or until notification
                        let timeout = std::time::Duration::from_secs(30);
                        idle_handle.wait_with_timeout(timeout)
                    }
                    Err(e) => {
                        debug_log(&format!("IDLE session: failed to start IDLE: {}", e));
                        return Err(EmailError::ImapError(e.to_string()));
                    }
                }
            };
            
            // Process IDLE result (IDLE handle is now dropped)
            match idle_result {
                Ok(_) => {
                    debug_log("IDLE session: received server notification");
                    consecutive_health_checks = 0; // Reset health check counter
                    
                    // Check current message count
                    let current_count = session.search("ALL")
                        .map_err(|e| EmailError::ImapError(e.to_string()))?
                        .len();
                    
                    debug_log(&format!("IDLE session: message count changed from {} to {}", 
                        last_known_count, current_count));
                    
                    if current_count != *last_known_count {
                        // Fetch new emails incrementally
                        match self.fetch_new_emails_since_count(folder, *last_known_count) {
                            Ok(new_emails) => {
                                if !new_emails.is_empty() {
                                    debug_log(&format!("IDLE session: saving {} new emails to database", new_emails.len()));
                                    if let Err(e) = database.save_emails(&self.account.email, folder, &new_emails) {
                                        debug_log(&format!("IDLE session: failed to save emails to database: {}", e));
                                    } else {
                                        debug_log("IDLE session: new emails saved to database");
                                    }
                                }
                            }
                            Err(e) => {
                                debug_log(&format!("IDLE session: failed to fetch new emails: {}", e));
                                // Continue IDLE loop even if fetch fails
                            }
                        }
                        *last_known_count = current_count;
                    }
                }
                Err(e) => {
                    debug_log(&format!("IDLE session: timeout or error: {}", e));
                    
                    // After timeout, check connection health
                    if !self.is_connection_healthy_secure(&mut session) {
                        consecutive_health_checks += 1;
                        debug_log(&format!("IDLE session: connection health check failed (attempt {})", consecutive_health_checks));
                        
                        if consecutive_health_checks >= MAX_HEALTH_CHECK_FAILURES {
                            debug_log("IDLE session: multiple health check failures, triggering reconnection");
                            return Err(EmailError::ImapError("Connection health check failed multiple times".to_string()));
                        }
                    } else {
                        consecutive_health_checks = 0; // Reset counter on successful health check
                        debug_log("IDLE session: connection healthy after timeout");
                    }
                }
            }
            
            // Small delay before next IDLE cycle
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    

    
    fn run_idle_session_plain(
        &self,
        folder: &str,
        database: &crate::database::EmailDatabase,
        running: &Arc<Mutex<bool>>,
    ) -> Result<(), EmailError> {
        let mut reconnect_attempts = 0;
        const MAX_RECONNECT_ATTEMPTS: u32 = 10; // Increased for suspend/resume scenarios
        let mut last_known_count = 0; // Track message count across reconnections
        
        loop {
            // Check if we should stop
            {
                let running_guard = running.lock().unwrap();
                if !*running_guard {
                    debug_log("IDLE session (plain): stopping due to running flag");
                    return Ok(());
                }
            }
            
            match self.run_single_idle_session_plain_with_count(folder, database, running, &mut last_known_count) {
                Ok(_) => {
                    debug_log("IDLE session (plain) completed normally");
                    return Ok(());
                }
                Err(e) => {
                    reconnect_attempts += 1;
                    debug_log(&format!("IDLE session (plain) error (attempt {}): {}", reconnect_attempts, e));
                    
                    if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                        debug_log("Max reconnection attempts reached, stopping IDLE session (plain)");
                        return Err(e);
                    }
                    
                    // Progressive backoff for reconnection attempts
                    let backoff_seconds = std::cmp::min(5 * reconnect_attempts, 60);
                    debug_log(&format!("Waiting {} seconds before reconnection attempt...", backoff_seconds));
                    std::thread::sleep(std::time::Duration::from_secs(backoff_seconds as u64));
                    
                    debug_log(&format!("Attempting to reconnect IDLE session (plain) (last known count: {})...", last_known_count));
                }
            }
        }
    }
    
    fn run_single_idle_session_plain_with_count(
        &self,
        folder: &str,
        database: &crate::database::EmailDatabase,
        running: &Arc<Mutex<bool>>,
        last_known_count: &mut usize,
    ) -> Result<(), EmailError> {
        let mut session = self.connect_imap_plain()?;
        session.select(folder)
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        debug_log("IDLE session (plain): connected and folder selected");
        
        // Check if server supports IDLE
        let caps = session.capabilities()
            .map_err(|e| EmailError::ImapError(e.to_string()))?;
        
        if !caps.has_str("IDLE") {
            debug_log("Server does not support IDLE, falling back to polling");
            return Err(EmailError::ImapError("Server does not support IDLE".to_string()));
        }
        
        debug_log("IDLE session (plain): Server supports IDLE, starting suspend/resume resilient IDLE loop");
        
        // Sync any missed emails from previous disconnection and update count
        // TODO: In new architecture, this is handled by sync daemon
        // *last_known_count = self.sync_emails_after_reconnection(folder, *last_known_count, tx)?;
        debug_log(&format!("IDLE session (plain): message count after reconnection sync: {}", last_known_count));
        
        // Main IDLE loop with shorter timeouts for better suspend/resume handling
        let mut consecutive_health_checks = 0;
        const MAX_HEALTH_CHECK_FAILURES: u32 = 3;
        
        loop {
            // Check if we should stop
            {
                let running_guard = running.lock().unwrap();
                if !*running_guard {
                    debug_log("IDLE session (plain): stopping due to running flag");
                    return Ok(());
                }
            }
            
            // Use shorter IDLE timeout (30 seconds) for better suspend/resume detection
            debug_log("IDLE session (plain): starting IDLE command with 30-second timeout");
            
            // Separate the IDLE operation to ensure proper scoping
            let idle_result = {
                match session.idle() {
                    Ok(idle_handle) => {
                        debug_log("IDLE session (plain): IDLE started, waiting for notifications or timeout");
                        
                        // Wait for 30 seconds or until notification
                        let timeout = std::time::Duration::from_secs(30);
                        idle_handle.wait_with_timeout(timeout)
                    }
                    Err(e) => {
                        debug_log(&format!("IDLE session (plain): failed to start IDLE: {}", e));
                        return Err(EmailError::ImapError(e.to_string()));
                    }
                }
            };
            
            // Process IDLE result (IDLE handle is now dropped)
            match idle_result {
                Ok(_) => {
                    debug_log("IDLE session (plain): received server notification");
                    consecutive_health_checks = 0; // Reset health check counter
                    
                    // Check current message count
                    let current_count = session.search("ALL")
                        .map_err(|e| EmailError::ImapError(e.to_string()))?
                        .len();
                    
                    debug_log(&format!("IDLE session (plain): message count changed from {} to {}", 
                        last_known_count, current_count));
                    
                    if current_count != *last_known_count {
                        // Fetch new emails incrementally
                        match self.fetch_new_emails_since_count(folder, *last_known_count) {
                            Ok(new_emails) => {
                                if !new_emails.is_empty() {
                                    debug_log(&format!("IDLE session (plain): saving {} new emails to database", new_emails.len()));
                                    if let Err(e) = database.save_emails(&self.account.email, folder, &new_emails) {
                                        debug_log(&format!("IDLE session (plain): failed to save emails to database: {}", e));
                                    } else {
                                        debug_log("IDLE session (plain): new emails saved to database");
                                    }
                                }
                            }
                            Err(e) => {
                                debug_log(&format!("IDLE session (plain): failed to fetch new emails: {}", e));
                                // Continue IDLE loop even if fetch fails
                            }
                        }
                        *last_known_count = current_count;
                    }
                }
                Err(e) => {
                    debug_log(&format!("IDLE session (plain): timeout or error: {}", e));
                    
                    // After timeout, check connection health
                    if !self.is_connection_healthy_plain(&mut session) {
                        consecutive_health_checks += 1;
                        debug_log(&format!("IDLE session (plain): connection health check failed (attempt {})", consecutive_health_checks));
                        
                        if consecutive_health_checks >= MAX_HEALTH_CHECK_FAILURES {
                            debug_log("IDLE session (plain): multiple health check failures, triggering reconnection");
                            return Err(EmailError::ImapError("Connection health check failed multiple times".to_string()));
                        }
                    } else {
                        consecutive_health_checks = 0; // Reset counter on successful health check
                        debug_log("IDLE session (plain): connection healthy after timeout");
                    }
                }
            }
            
            // Small delay before next IDLE cycle
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    


    #[allow(dead_code)]
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
    
    /// Get the latest UID from the server (lightweight check for new mail)
    pub fn get_latest_uid(&self, folder: &str) -> Result<u32, EmailError> {
        debug_log(&format!("get_latest_uid called for folder: {}", folder));
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;

                // Select folder
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(format!("Failed to select folder {}: {}", folder, e)))?;

                // Get the highest UID using SEARCH
                let search_result = session.search("ALL")
                    .map_err(|e| EmailError::ImapError(format!("Failed to search emails: {}", e)))?;

                let latest_uid = if search_result.is_empty() {
                    0
                } else {
                    *search_result.iter().max().unwrap_or(&0)
                };

                debug_log(&format!("Latest UID for folder {}: {}", folder, latest_uid));

                // Logout
                let _ = session.logout();

                Ok(latest_uid)
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;

                // Select folder
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(format!("Failed to select folder {}: {}", folder, e)))?;

                // Get the highest UID using SEARCH
                let search_result = session.search("ALL")
                    .map_err(|e| EmailError::ImapError(format!("Failed to search emails: {}", e)))?;

                let latest_uid = if search_result.is_empty() {
                    0
                } else {
                    *search_result.iter().max().unwrap_or(&0)
                };

                debug_log(&format!("Latest UID for folder {}: {}", folder, latest_uid));

                // Logout
                let _ = session.logout();

                Ok(latest_uid)
            }
        }
    }
    
    /// Fetch emails since a specific UID (for incremental sync)
    pub fn fetch_emails_since_uid(&self, folder: &str, since_uid: u32) -> Result<Vec<Email>, EmailError> {
        debug_log(&format!("fetch_emails_since_uid called: folder='{}', since_uid={}", folder, since_uid));
        
        match self.account.imap_security {
            ImapSecurity::SSL | ImapSecurity::StartTLS => {
                let mut session = self.connect_imap_secure()?;

                // Select folder
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(format!("Failed to select folder {}: {}", folder, e)))?;

                // Search for emails with UID greater than since_uid
                let search_query = format!("UID {}:*", since_uid);
                let search_result = session.search(&search_query)
                    .map_err(|e| EmailError::ImapError(format!("Failed to search emails since UID {}: {}", since_uid, e)))?;

                if search_result.is_empty() {
                    debug_log("No new emails found");
                    let _ = session.logout();
                    return Ok(vec![]);
                }

                debug_log(&format!("Found {} new emails since UID {}", search_result.len(), since_uid));

                // Fetch the new emails
                let sequence_set = search_result.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
                let messages = session.fetch(&sequence_set, "RFC822 FLAGS")
                    .map_err(|e| EmailError::ImapError(format!("Failed to fetch new emails: {}", e)))?;

                let mut emails = Vec::new();
                for message in messages.iter() {
                    if let Some(body) = message.body() {
                        match mail_parser::Message::parse(body) {
                            Some(parsed) => {
                                let uid = message.uid.unwrap_or(0).to_string();
                                let flags = message.flags().iter().map(|f| f.to_string()).collect();
                                
                                match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                                    Ok(email) => emails.push(email),
                                    Err(e) => debug_log(&format!("Failed to parse email {}: {}", uid, e)),
                                }
                            }
                            None => debug_log("Failed to parse email body"),
                        }
                    }
                }

                debug_log(&format!("Successfully parsed {} new emails", emails.len()));

                // Logout
                let _ = session.logout();

                Ok(emails)
            }
            ImapSecurity::None => {
                let mut session = self.connect_imap_plain()?;

                // Select folder
                session.select(folder)
                    .map_err(|e| EmailError::ImapError(format!("Failed to select folder {}: {}", folder, e)))?;

                // Search for emails with UID greater than since_uid
                let search_query = format!("UID {}:*", since_uid);
                let search_result = session.search(&search_query)
                    .map_err(|e| EmailError::ImapError(format!("Failed to search emails since UID {}: {}", since_uid, e)))?;

                if search_result.is_empty() {
                    debug_log("No new emails found");
                    let _ = session.logout();
                    return Ok(vec![]);
                }

                debug_log(&format!("Found {} new emails since UID {}", search_result.len(), since_uid));

                // Fetch the new emails
                let sequence_set = search_result.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
                let messages = session.fetch(&sequence_set, "RFC822 FLAGS")
                    .map_err(|e| EmailError::ImapError(format!("Failed to fetch new emails: {}", e)))?;

                let mut emails = Vec::new();
                for message in messages.iter() {
                    if let Some(body) = message.body() {
                        match mail_parser::Message::parse(body) {
                            Some(parsed) => {
                                let uid = message.uid.unwrap_or(0).to_string();
                                let flags = message.flags().iter().map(|f| f.to_string()).collect();
                                
                                match Email::from_parsed_email(&parsed, &uid, folder, flags) {
                                    Ok(email) => emails.push(email),
                                    Err(e) => debug_log(&format!("Failed to parse email {}: {}", uid, e)),
                                }
                            }
                            None => debug_log("Failed to parse email body"),
                        }
                    }
                }

                debug_log(&format!("Successfully parsed {} new emails", emails.len()));

                // Logout
                let _ = session.logout();

                Ok(emails)
            }
        }
    }
}

// Background email fetcher with IMAP IDLE support
#[allow(dead_code)]
pub struct EmailFetcher {
    client: EmailClient,
    tx: mpsc::Sender<Vec<Email>>,
    interval: std::time::Duration,
    running: Arc<Mutex<bool>>,
    handle: Option<std::thread::JoinHandle<()>>,
    use_idle: bool,
}

impl EmailFetcher {
    #[allow(dead_code)]
    pub fn new(
        client: EmailClient, 
        tx: mpsc::Sender<Vec<Email>>,
        interval_secs: u64,
    ) -> Self {
        // Check if server supports IDLE
        let use_idle = client.supports_idle();
        debug_log(&format!("Server IDLE support: {}", use_idle));
        
        Self {
            client,
            tx,
            interval: std::time::Duration::from_secs(interval_secs),
            running: Arc::new(Mutex::new(false)),
            handle: None,
            use_idle,
        }
    }
    
    #[allow(dead_code)]
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
                match client.fetch_emails(&current_folder, 200) {
                    Ok(emails) => {
                        // Try to send emails
                        if let Err(e) = tx.send(emails) {
                            // Receiver is closed, exit the loop
                            debug_log(&format!("Email channel closed: {}", e));
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch emails: {}", e);
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
                log::error!("Error joining email fetcher thread: {:?}", e);
            }
        }
    }
    
    #[allow(dead_code)]
    pub fn set_folder(&self, folder: String) {
        // For now, we'll keep it simple and just use INBOX
        // In a more advanced implementation, we could use channels to communicate
        // folder changes to the background thread
        log::debug!("Folder change requested: {}", folder);
    }
}

impl Drop for EmailFetcher {
    fn drop(&mut self) {
        self.stop();
    }
}
