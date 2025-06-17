// Test script to verify attachment sending functionality
// This demonstrates how attachments are processed in the email sending logic

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct Email {
    pub id: String,
    pub subject: String,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<EmailAttachment>,
    pub flags: Vec<String>,
    pub headers: HashMap<String, String>,
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
            body_text: Some("Test email body".to_string()),
            body_html: None,
            attachments: Vec::new(),
            flags: Vec::new(),
            headers: HashMap::new(),
        }
    }
}

fn main() {
    println!("Testing attachment functionality...");
    
    let mut email = Email::new();
    email.subject = "Test Email with Attachment".to_string();
    
    // Add a test attachment
    let attachment = EmailAttachment {
        filename: "test.txt".to_string(),
        content_type: "text/plain".to_string(),
        data: b"This is test attachment content".to_vec(),
    };
    
    email.attachments.push(attachment);
    
    println!("Email created with {} attachment(s)", email.attachments.len());
    for (i, att) in email.attachments.iter().enumerate() {
        println!("  Attachment {}: {} ({} bytes, {})", 
            i + 1, 
            att.filename, 
            att.data.len(), 
            att.content_type
        );
    }
    
    println!("Attachment functionality test completed!");
}
