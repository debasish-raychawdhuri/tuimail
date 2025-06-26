# IMAP Compatibility Fix for Smart Sync

## Problem
The IMAP SEARCH SINCE command might not be supported by all email servers, or the date format might be incompatible, causing "bad imap response" errors.

## Solution
Implement a fallback strategy that uses more compatible IMAP commands:

1. **Try SEARCH SINCE first** (current approach)
2. **Fallback to UID SEARCH** if SINCE fails
3. **Fallback to simple UID fetch** if search fails entirely

## Implementation

```rust
fn sync_emails_since_date_with_fallback(&self, folder: &str, since_date: DateTime<Utc>, database: &EmailDatabase) -> Result<Vec<Email>, EmailError> {
    // Try different approaches in order of preference
    
    // Approach 1: SEARCH SINCE (most efficient)
    if let Ok(emails) = self.try_search_since(folder, since_date, database) {
        return Ok(emails);
    }
    
    // Approach 2: UID SEARCH with date range
    if let Ok(emails) = self.try_uid_search_date(folder, since_date, database) {
        return Ok(emails);
    }
    
    // Approach 3: Fetch recent UIDs and check dates
    if let Ok(emails) = self.try_recent_uid_fetch(folder, database) {
        return Ok(emails);
    }
    
    // Approach 4: Fallback to old method
    debug_log("All smart sync approaches failed, falling back to traditional fetch");
    self.fetch_emails_traditional(folder, 100) // Limit to recent 100 emails
}

fn try_search_since(&self, folder: &str, since_date: DateTime<Utc>, database: &EmailDatabase) -> Result<Vec<Email>, EmailError> {
    let search_date = since_date.format("%d-%b-%Y").to_string();
    debug_log(&format!("Trying SEARCH SINCE: {}", search_date));
    
    match self.account.imap_security {
        ImapSecurity::SSL | ImapSecurity::StartTLS => {
            let mut session = self.connect_imap_secure()?;
            session.select(folder)?;
            
            match session.search(&format!("SINCE {}", search_date)) {
                Ok(search_result) => {
                    debug_log(&format!("SEARCH SINCE succeeded: {} results", search_result.len()));
                    self.process_search_results(&mut session, folder, search_result, database)
                }
                Err(e) => {
                    debug_log(&format!("SEARCH SINCE failed: {}", e));
                    Err(EmailError::ImapError(e.to_string()))
                }
            }
        }
        ImapSecurity::None => {
            let mut session = self.connect_imap_plain()?;
            session.select(folder)?;
            
            match session.search(&format!("SINCE {}", search_date)) {
                Ok(search_result) => {
                    debug_log(&format!("SEARCH SINCE succeeded: {} results", search_result.len()));
                    self.process_search_results(&mut session, folder, search_result, database)
                }
                Err(e) => {
                    debug_log(&format!("SEARCH SINCE failed: {}", e));
                    Err(EmailError::ImapError(e.to_string()))
                }
            }
        }
    }
}

fn try_uid_search_date(&self, folder: &str, since_date: DateTime<Utc>, database: &EmailDatabase) -> Result<Vec<Email>, EmailError> {
    debug_log("Trying UID SEARCH with date criteria");
    
    match self.account.imap_security {
        ImapSecurity::SSL | ImapSecurity::StartTLS => {
            let mut session = self.connect_imap_secure()?;
            session.select(folder)?;
            
            // Try different date formats
            let date_formats = vec![
                since_date.format("%d-%b-%Y").to_string(),
                since_date.format("%d-%m-%Y").to_string(),
                since_date.format("%Y-%m-%d").to_string(),
            ];
            
            for date_format in date_formats {
                match session.uid_search(&format!("SINCE {}", date_format)) {
                    Ok(search_result) => {
                        debug_log(&format!("UID SEARCH succeeded with format {}: {} results", date_format, search_result.len()));
                        return self.process_uid_search_results(&mut session, folder, search_result, database);
                    }
                    Err(e) => {
                        debug_log(&format!("UID SEARCH failed with format {}: {}", date_format, e));
                    }
                }
            }
            
            Err(EmailError::ImapError("All UID SEARCH date formats failed".to_string()))
        }
        ImapSecurity::None => {
            // Similar implementation for plain connection
            Err(EmailError::ImapError("UID SEARCH fallback not implemented for plain".to_string()))
        }
    }
}

fn try_recent_uid_fetch(&self, folder: &str, database: &EmailDatabase) -> Result<Vec<Email>, EmailError> {
    debug_log("Trying recent UID fetch approach");
    
    match self.account.imap_security {
        ImapSecurity::SSL | ImapSecurity::StartTLS => {
            let mut session = self.connect_imap_secure()?;
            session.select(folder)?;
            
            // Get the highest UID from database
            let last_known_uid = database.get_latest_uid(&self.account.email, folder).unwrap_or(1);
            
            // Fetch emails with UIDs higher than what we have
            let uid_range = format!("{}:*", last_known_uid + 1);
            
            match session.uid_fetch(&uid_range, "RFC822 FLAGS UID") {
                Ok(messages) => {
                    debug_log(&format!("Recent UID fetch succeeded: {} messages", messages.len()));
                    self.process_fetched_messages(messages, folder, database)
                }
                Err(e) => {
                    debug_log(&format!("Recent UID fetch failed: {}", e));
                    Err(EmailError::ImapError(e.to_string()))
                }
            }
        }
        ImapSecurity::None => {
            // Similar implementation for plain connection
            Err(EmailError::ImapError("Recent UID fetch not implemented for plain".to_string()))
        }
    }
}
```

## Database Helper

```rust
impl EmailDatabase {
    pub fn get_latest_uid(&self, account_email: &str, folder: &str) -> Result<u32> {
        let result = self.conn.query_row(
            "SELECT MAX(uid) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get::<_, Option<u32>>(0)
        );
        
        match result {
            Ok(Some(uid)) => Ok(uid),
            Ok(None) => Ok(0), // No emails found
            Err(e) => Err(e.into())
        }
    }
}
```

## Integration

Replace the current `sync_emails_since_date` function with the fallback version:

```rust
fn sync_emails_since_date(&self, folder: &str, since_date: DateTime<Utc>) -> Result<Vec<Email>, EmailError> {
    let database = EmailDatabase::new(&self.db_path)
        .map_err(|e| EmailError::ImapError(format!("Failed to open database: {}", e)))?;
    
    self.sync_emails_since_date_with_fallback(folder, since_date, &database)
}
```

This approach provides multiple fallback strategies to ensure compatibility with different IMAP servers and configurations.
