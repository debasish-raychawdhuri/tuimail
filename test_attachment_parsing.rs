// Test attachment parsing with a manually created email
use std::collections::HashMap;

// Simulate the email structures
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

fn main() {
    println!("Testing attachment parsing logic...");
    
    // Create a simple multipart email with attachment (RFC822 format)
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
It has multiple lines.
And some test data.

--boundary123--
"#;

    println!("Sample email structure:");
    println!("{}", email_with_attachment);
    
    // Try to parse with mail_parser
    match mail_parser::Message::parse(email_with_attachment.as_bytes()) {
        Some(parsed) => {
            println!("\n✅ Email parsed successfully!");
            println!("Parts: {}", parsed.parts.len());
            
            for (i, part) in parsed.parts.iter().enumerate() {
                println!("\nPart {}: {} headers", i, part.headers.len());
                
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
                    println!("  Header: {} = {}", header_name, header_value);
                }
                
                // Check body type
                match &part.body {
                    mail_parser::PartType::Text(text) => {
                        println!("  Body: Text ({} chars)", text.len());
                        if text.len() < 100 {
                            println!("    Content: {}", text);
                        }
                    }
                    mail_parser::PartType::Html(html) => {
                        println!("  Body: HTML ({} chars)", html.len());
                    }
                    mail_parser::PartType::Binary(binary) => {
                        println!("  Body: Binary ({} bytes)", binary.len());
                    }
                    mail_parser::PartType::InlineBinary(binary) => {
                        println!("  Body: Inline Binary ({} bytes)", binary.len());
                    }
                    mail_parser::PartType::Message(_) => {
                        println!("  Body: Nested Message");
                    }
                    mail_parser::PartType::Multipart(_) => {
                        println!("  Body: Multipart Container");
                    }
                }
            }
            
            // Test our attachment detection logic
            println!("\n🔍 Testing attachment detection...");
            let mut attachment_count = 0;
            
            for (i, part) in parsed.parts.iter().enumerate() {
                let mut is_attachment = false;
                let mut filename = None;
                
                for header in &part.headers {
                    let header_name_str = format!("{:?}", header.name).to_lowercase();
                    
                    let header_value = match &header.value {
                        mail_parser::HeaderValue::Text(text) => text.as_ref(),
                        mail_parser::HeaderValue::TextList(list) => {
                            if let Some(first) = list.first() {
                                first.as_ref()
                            } else {
                                continue;
                            }
                        }
                        _ => continue,
                    };
                    
                    if header_name_str.contains("contentdisposition") || header_name_str.contains("content-disposition") {
                        if header_value.contains("attachment") {
                            is_attachment = true;
                            if let Some(start) = header_value.find("filename=") {
                                let filename_part = &header_value[start + 9..];
                                let filename_part = filename_part.trim_start_matches('"');
                                if let Some(end) = filename_part.find('"') {
                                    filename = Some(filename_part[..end].to_string());
                                } else {
                                    filename = Some(filename_part.trim().to_string());
                                }
                            }
                        }
                    }
                }
                
                if is_attachment {
                    attachment_count += 1;
                    println!("  ✅ Part {} is an attachment: {:?}", i, filename);
                } else {
                    println!("  ❌ Part {} is not an attachment", i);
                }
            }
            
            println!("\n📎 Total attachments found: {}", attachment_count);
            
            if attachment_count > 0 {
                println!("✅ Attachment parsing logic works!");
            } else {
                println!("❌ No attachments detected - check parsing logic");
            }
        }
        None => {
            println!("❌ Failed to parse email");
        }
    }
}
