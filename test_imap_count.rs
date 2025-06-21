use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing IMAP Server Message Counts");
    println!("=====================================");
    
    // Load config
    let config_path = format!("{}/.config/tuimail/config.json", 
        dirs::home_dir().unwrap().display());
    let config_content = std::fs::read_to_string(&config_path)?;
    let config: serde_json::Value = serde_json::from_str(&config_content)?;
    
    // Get the 214054001@iitb.ac.in account
    let accounts = config["accounts"].as_array().unwrap();
    let target_account = accounts.iter()
        .find(|acc| acc["email"].as_str() == Some("214054001@iitb.ac.in"))
        .expect("Account not found");
    
    println!("📧 Account: {}", target_account["email"].as_str().unwrap());
    println!("🏠 Server: {}", target_account["imap_server"].as_str().unwrap());
    
    // Connect to IMAP and check actual counts
    let domain = "imap.gmail.com";
    let port = 993;
    
    println!("\n🔌 Connecting to IMAP server...");
    
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((domain, port), domain, &tls)?;
    
    // Get credentials (you'll need to enter password)
    println!("🔐 Enter password for 214054001@iitb.ac.in:");
    let password = rpassword::read_password()?;
    
    let mut session = client.login("214054001@iitb.ac.in", &password)
        .map_err(|e| format!("Login failed: {:?}", e))?;
    
    println!("✅ Connected successfully!");
    
    // Check different folders
    let folders = vec!["INBOX", "Sent", "[Gmail]/All Mail", "[Gmail]/Sent Mail"];
    
    for folder in folders {
        match session.select(folder) {
            Ok(mailbox) => {
                println!("\n📁 Folder: {}", folder);
                println!("   📊 Total messages: {}", mailbox.exists);
                println!("   🆔 UIDNEXT: {}", mailbox.uid_next.unwrap_or(0));
                println!("   🔢 UIDVALIDITY: {}", mailbox.uid_validity.unwrap_or(0));
                
                // Check UID range
                if mailbox.exists > 0 {
                    // Get first and last UIDs
                    let first_uid = session.uid_fetch("1", "UID")?;
                    let last_uid = session.uid_fetch(&format!("{}", mailbox.exists), "UID")?;
                    
                    if let (Some(first), Some(last)) = (first_uid.first(), last_uid.first()) {
                        println!("   🔢 UID Range: {} - {}", 
                            first.uid.unwrap_or(0), 
                            last.uid.unwrap_or(0));
                    }
                }
            }
            Err(e) => {
                println!("\n❌ Failed to select folder {}: {}", folder, e);
            }
        }
    }
    
    session.logout()?;
    println!("\n✅ IMAP test completed!");
    
    Ok(())
}
