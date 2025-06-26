# Smart Sync Strategy - TUImail

## Sync Logic Based on Database State

### Case 1: Fresh Install (No Database/No Emails)
- **Action**: Full initial sync - download recent emails (e.g., last 30 days)
- **Reason**: App is starting for the first time

### Case 2: Existing Database with Emails
- **Action**: Incremental sync from last email timestamp
- **Reason**: App has been used before, only sync changes

### Case 3: Database Exists but Empty Folder
- **Action**: Full sync for that specific folder
- **Reason**: First time accessing this folder

## Implementation

```rust
pub fn determine_sync_strategy(&self, folder: &str) -> Result<SyncStrategy, EmailError> {
    let account_email = &self.account.email;
    
    // Check if we have any emails in database for this folder
    match self.database.get_email_count(account_email, folder) {
        Ok(0) => {
            // No emails in database - full initial sync
            debug_log(&format!("No emails found for {}/{} - performing initial sync", account_email, folder));
            Ok(SyncStrategy::InitialSync { days_back: 30 })
        }
        Ok(count) => {
            // We have emails - find the most recent timestamp
            match self.database.get_latest_email_timestamp(account_email, folder) {
                Ok(Some(latest_timestamp)) => {
                    debug_log(&format!("Found {} emails, latest from timestamp {} - performing incremental sync", 
                        count, latest_timestamp));
                    Ok(SyncStrategy::IncrementalSync { since_timestamp: latest_timestamp })
                }
                Ok(None) => {
                    // Emails exist but no valid timestamp - fallback to recent sync
                    debug_log("Emails exist but no valid timestamp - syncing last 7 days");
                    Ok(SyncStrategy::RecentSync { days_back: 7 })
                }
                Err(e) => {
                    debug_log(&format!("Error getting latest timestamp: {} - fallback to recent sync", e));
                    Ok(SyncStrategy::RecentSync { days_back: 7 })
                }
            }
        }
        Err(e) => {
            debug_log(&format!("Error checking email count: {} - performing initial sync", e));
            Ok(SyncStrategy::InitialSync { days_back: 30 })
        }
    }
}

#[derive(Debug)]
pub enum SyncStrategy {
    InitialSync { days_back: i64 },
    IncrementalSync { since_timestamp: i64 },
    RecentSync { days_back: i64 },
}
```

### Database Helper Functions

Add these to `src/database.rs`:

```rust
impl EmailDatabase {
    pub fn get_email_count(&self, account_email: &str, folder: &str) -> Result<i64> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get(0)
        )?;
        Ok(count)
    }
    
    pub fn get_latest_email_timestamp(&self, account_email: &str, folder: &str) -> Result<Option<i64>> {
        let result = self.conn.query_row(
            "SELECT MAX(date_received) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get::<_, Option<i64>>(0)
        );
        
        match result {
            Ok(timestamp) => Ok(timestamp),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }
    
    pub fn get_oldest_email_timestamp(&self, account_email: &str, folder: &str) -> Result<Option<i64>> {
        let result = self.conn.query_row(
            "SELECT MIN(date_received) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get::<_, Option<i64>>(0)
        );
        
        match result {
            Ok(timestamp) => Ok(timestamp),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }
}
```

### Smart Sync Implementation

```rust
pub fn smart_sync(&self, folder: &str) -> Result<Vec<Email>, EmailError> {
    let strategy = self.determine_sync_strategy(folder)?;
    
    match strategy {
        SyncStrategy::InitialSync { days_back } => {
            debug_log(&format!("Performing initial sync - last {} days", days_back));
            self.sync_recent_emails(folder, days_back)
        }
        
        SyncStrategy::IncrementalSync { since_timestamp } => {
            debug_log(&format!("Performing incremental sync since timestamp {}", since_timestamp));
            
            // Convert timestamp to date for IMAP search
            let since_date = DateTime::<Utc>::from_timestamp(since_timestamp, 0)
                .unwrap_or_else(|| Utc::now() - chrono::Duration::days(1));
            
            self.sync_emails_since_date(folder, since_date)
        }
        
        SyncStrategy::RecentSync { days_back } => {
            debug_log(&format!("Performing recent sync - last {} days", days_back));
            self.sync_recent_emails(folder, days_back)
        }
    }
}

fn sync_recent_emails(&self, folder: &str, days_back: i64) -> Result<Vec<Email>, EmailError> {
    let since_date = Utc::now() - chrono::Duration::days(days_back);
    self.sync_emails_since_date(folder, since_date)
}

fn sync_emails_since_date(&self, folder: &str, since_date: DateTime<Utc>) -> Result<Vec<Email>, EmailError> {
    let mut session = self.connect()?;
    session.select(folder)?;
    
    // Format date for IMAP SEARCH (DD-MMM-YYYY format)
    let search_date = since_date.format("%d-%b-%Y").to_string();
    debug_log(&format!("Searching for emails since: {}", search_date));
    
    // Search for emails since the given date
    let search_result = session.search(&format!("SINCE {}", search_date))?;
    
    if search_result.is_empty() {
        debug_log("No new emails found");
        return Ok(Vec::new());
    }
    
    debug_log(&format!("Found {} emails to sync", search_result.len()));
    
    let mut new_emails = Vec::new();
    let mut updated_flags = Vec::new();
    
    // Process in batches to avoid server limits
    for batch in search_result.chunks(50) {
        let sequence_set = batch.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        let messages = session.fetch(&sequence_set, "RFC822 FLAGS UID")?;
        
        for message in messages.iter() {
            if let Some(uid) = message.uid {
                let uid_str = uid.to_string();
                
                // Check if email already exists in database
                match self.database.email_exists(&self.account.email, folder, &uid_str) {
                    Ok(true) => {
                        // Email exists - check for flag changes
                        let server_flags: Vec<String> = message.flags().iter().map(|f| f.to_string()).collect();
                        let server_seen = server_flags.iter().any(|f| f == "\\Seen");
                        
                        // Update flag if different
                        if let Err(e) = self.database.update_email_seen_status(&self.account.email, folder, uid, server_seen) {
                            debug_log(&format!("Failed to update flag for UID {}: {}", uid_str, e));
                        } else {
                            updated_flags.push((uid_str, server_seen));
                        }
                    }
                    Ok(false) => {
                        // New email - parse and save
                        if let Some(body) = message.body() {
                            if let Some(parsed) = mail_parser::Message::parse(body) {
                                let flags: Vec<String> = message.flags().iter().map(|f| f.to_string()).collect();
                                match Email::from_parsed_email(&parsed, &uid_str, folder, flags) {
                                    Ok(email) => {
                                        // Save to database
                                        if let Err(e) = self.database.save_email(&email) {
                                            debug_log(&format!("Failed to save email {}: {}", uid_str, e));
                                        } else {
                                            new_emails.push(email);
                                        }
                                    }
                                    Err(e) => {
                                        debug_log(&format!("Failed to parse email {}: {}", uid_str, e));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug_log(&format!("Error checking if email exists {}: {}", uid_str, e));
                    }
                }
            }
        }
        
        // Small delay between batches
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    debug_log(&format!("Sync completed: {} new emails, {} flag updates", new_emails.len(), updated_flags.len()));
    Ok(new_emails)
}
```

### Add Helper to Database

```rust
impl EmailDatabase {
    pub fn email_exists(&self, account_email: &str, folder: &str, uid: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE account_email = ?1 AND folder = ?2 AND uid = ?3",
            params![account_email, folder, uid],
            |row| row.get(0)
        )?;
        Ok(count > 0)
    }
}
```

## Benefits of This Approach

- ✅ **Smart Initial Sync**: Only downloads recent emails on first run
- ✅ **Efficient Updates**: Only syncs changes since last email
- ✅ **Handles Edge Cases**: Graceful fallback for database issues
- ✅ **Scalable**: Works with mailboxes of any size
- ✅ **Fast Startup**: Quick sync based on existing data
- ✅ **Flag Sync**: Updates read status for existing emails

## Usage in App

```rust
// In src/app.rs refresh function
pub fn refresh_emails(&mut self) -> AppResult<()> {
    if let Some(client) = self.get_current_email_client() {
        match client.smart_sync(&self.current_folder) {
            Ok(new_emails) => {
                // Add new emails to UI
                self.emails.extend(new_emails);
                // Sort by date if needed
                self.emails.sort_by(|a, b| b.date_received.cmp(&a.date_received));
            }
            Err(e) => {
                self.show_error(&format!("Sync failed: {}", e));
            }
        }
    }
    Ok(())
}
```

This approach intelligently handles both initial sync and incremental updates based on the current state of your database!
