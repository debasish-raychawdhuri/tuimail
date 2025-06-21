use tuimail::{Config, EmailClient, EmailDatabase};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Manual Sync Test ===");
    
    // Load config
    let config = Config::load()?;
    if config.accounts.is_empty() {
        println!("No accounts configured");
        return Ok(());
    }
    
    let account = &config.accounts[0]; // First account
    println!("Testing account: {}", account.email);
    
    // Create database
    let db_path = format!("{}/.cache/tuimail/emails.db", std::env::var("HOME")?);
    let database = Arc::new(EmailDatabase::new(&db_path)?);
    
    // Create email client
    let credentials = tuimail::credentials::SecureCredentials::new()?;
    let client = EmailClient::new(account.clone(), credentials);
    
    // Check current count
    let current_count = database.get_all_emails(&account.email, "INBOX")?.len();
    println!("Current emails in database: {}", current_count);
    
    // Fetch emails
    println!("Fetching emails from server...");
    let emails = client.fetch_emails("INBOX", 0)?; // 0 = no limit
    println!("Fetched {} emails from server", emails.len());
    
    // Save to database
    database.save_emails(&account.email, "INBOX", &emails)?;
    
    // Check new count
    let new_count = database.get_all_emails(&account.email, "INBOX")?.len();
    println!("New emails in database: {}", new_count);
    
    println!("=== Test Complete ===");
    Ok(())
}
