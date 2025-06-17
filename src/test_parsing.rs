// Test module for attachment parsing
use crate::email::Email;

pub fn test_attachment_parsing() {
    println!("Testing attachment parsing with sample email...");
    
    // Enable debug logging for this test
    std::env::set_var("EMAIL_DEBUG", "1");
    
    // Create a simple multipart email with attachment
    let email_with_attachment = r#"From: test@example.com
To: recipient@example.com
Subject: Test Email with Attachment
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary="boundary123"

--boundary123
Content-Type: text/plain

This is the email body with an attachment.

--boundary123
Content-Type: text/plain; name="test.txt"
Content-Disposition: attachment; filename="test.txt"

This is the content of the attached file.

--boundary123--
"#;

    match mail_parser::Message::parse(email_with_attachment.as_bytes()) {
        Some(parsed) => {
            println!("✅ Email parsed successfully!");
            println!("Parts: {}", parsed.parts.len());
            
            // Test our attachment extraction
            match Email::from_parsed_email(&parsed, "test123", "INBOX", vec![]) {
                Ok(email) => {
                    println!("✅ Email object created successfully!");
                    println!("Subject: {}", email.subject);
                    println!("Attachments: {}", email.attachments.len());
                    
                    for (i, att) in email.attachments.iter().enumerate() {
                        println!("  Attachment {}: {} ({} bytes, {})", 
                            i + 1, att.filename, att.data.len(), att.content_type);
                    }
                    
                    if email.attachments.len() > 0 {
                        println!("🎉 Attachment parsing works!");
                    } else {
                        println!("❌ No attachments found - check debug output above");
                    }
                }
                Err(e) => {
                    println!("❌ Failed to create email object: {}", e);
                }
            }
        }
        None => {
            println!("❌ Failed to parse email");
        }
    }
}
