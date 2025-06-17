// Test script to send an email with attachment to ourselves for testing
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
            subject: "Test Email with Attachment".to_string(),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            body_text: Some("This is a test email with an attachment to verify the attachment parsing functionality.".to_string()),
            body_html: None,
            attachments: Vec::new(),
            flags: Vec::new(),
            headers: HashMap::new(),
        }
    }
}

fn main() {
    println!("Creating test email with attachment...");
    
    let mut email = Email::new();
    
    // Add a test attachment
    let attachment_content = b"This is test attachment content for debugging attachment parsing.\nIt contains multiple lines and some special characters: !@#$%^&*()";
    let attachment = EmailAttachment {
        filename: "test_debug.txt".to_string(),
        content_type: "text/plain".to_string(),
        data: attachment_content.to_vec(),
    };
    
    email.attachments.push(attachment);
    
    println!("Test email created:");
    println!("  Subject: {}", email.subject);
    println!("  Body: {:?}", email.body_text);
    println!("  Attachments: {}", email.attachments.len());
    
    for (i, att) in email.attachments.iter().enumerate() {
        println!("    Attachment {}: {} ({} bytes, {})", 
            i + 1, att.filename, att.data.len(), att.content_type);
        println!("    Content preview: {}", 
            String::from_utf8_lossy(&att.data[..50.min(att.data.len())]));
    }
    
    println!("\nThis email structure should be sent to test attachment parsing.");
    println!("Use the email client to compose and send this type of email to yourself.");
}
