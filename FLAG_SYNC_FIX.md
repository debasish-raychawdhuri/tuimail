# Flag Synchronization Fix - TUImail

## Problem
The email client is not properly synchronizing email flags (read/unread status) between the IMAP server and local database. This causes:

1. **Read status not updated to server**: When opening emails, read status isn't always marked on server
2. **Server changes not reflected**: When other clients mark emails as read, TUImail doesn't reflect these changes
3. **Inconsistent state**: Local database and server have different flag states

## Root Causes

1. **No flag synchronization during fetch**: Email fetching gets flags but doesn't update existing emails
2. **Missing flag sync function**: No dedicated function to sync flags for existing emails
3. **No periodic flag updates**: Background sync doesn't include flag synchronization

## Solution

### 1. Add Flag Synchronization Function

Add a new function to sync flags for existing emails:

```rust
pub fn sync_email_flags(&self, folder: &str) -> Result<(), EmailError> {
    debug_log(&format!("Starting flag sync for folder: {}", folder));
    
    let mut session = self.connect()?;
    session.select(folder)?;
    
    // Get all email UIDs from database
    let db_emails = self.database.get_all_emails(&self.account.email, folder)?;
    
    if db_emails.is_empty() {
        return Ok(());
    }
    
    // Create UID sequence for FETCH
    let uids: Vec<String> = db_emails.iter().map(|e| e.id.clone()).collect();
    let uid_sequence = uids.join(",");
    
    // Fetch current flags from server
    let messages = session.uid_fetch(&uid_sequence, "FLAGS")?;
    
    for message in messages.iter() {
        if let Some(uid) = message.uid {
            let uid_str = uid.to_string();
            let server_flags: Vec<String> = message.flags().iter().map(|f| f.to_string()).collect();
            let server_seen = server_flags.iter().any(|f| f == "\\Seen");
            
            // Find corresponding email in database
            if let Some(db_email) = db_emails.iter().find(|e| e.id == uid_str) {
                if db_email.seen != server_seen {
                    debug_log(&format!("Updating flag for UID {}: {} -> {}", uid_str, db_email.seen, server_seen));
                    
                    // Update database with server flag state
                    self.database.update_email_seen_status(&self.account.email, folder, uid, server_seen)?;
                }
            }
        }
    }
    
    debug_log("Flag sync completed");
    Ok(())
}
```

### 2. Integrate Flag Sync into Refresh

Modify the refresh functionality to include flag synchronization:

```rust
pub fn refresh_emails(&self, folder: &str) -> Result<Vec<Email>, EmailError> {
    // First sync flags for existing emails
    if let Err(e) = self.sync_email_flags(folder) {
        debug_log(&format!("Flag sync failed: {}", e));
    }
    
    // Then fetch new emails
    self.fetch_emails(folder, 0)
}
```

### 3. Add Flag Sync to Background Thread

Include flag synchronization in the background sync process:

```rust
// In background sync thread
if let Err(e) = client.sync_email_flags(&current_folder) {
    debug_log(&format!("Background flag sync failed: {}", e));
}
```

### 4. Improve Mark as Read Function

Ensure mark as read operations are properly queued and executed:

```rust
pub fn mark_as_read_with_retry(&self, email: &Email) -> Result<(), EmailError> {
    let max_retries = 3;
    let mut last_error = None;
    
    for attempt in 1..=max_retries {
        match self.mark_as_read(email) {
            Ok(_) => {
                // Update local database immediately
                self.database.update_email_seen_status(&self.account.email, &email.folder, 
                    email.id.parse().unwrap_or(0), true)?;
                return Ok(());
            }
            Err(e) => {
                debug_log(&format!("Mark as read attempt {} failed: {}", attempt, e));
                last_error = Some(e);
                if attempt < max_retries {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    }
    
    Err(last_error.unwrap())
}
```

## Implementation Steps

1. **Add flag sync function** to `EmailClient` in `src/email.rs`
2. **Modify refresh logic** in `src/app.rs` to call flag sync
3. **Update background sync** to include flag synchronization
4. **Improve error handling** for flag operations
5. **Add debug logging** for flag sync operations

## Testing

1. **Test read status sync**:
   - Mark email as read in another client
   - Press 'r' in TUImail
   - Verify read status is updated

2. **Test mark as read**:
   - Open unread email in TUImail
   - Check server shows email as read
   - Verify database is updated

3. **Test background sync**:
   - Leave TUImail running
   - Mark emails as read in another client
   - Verify TUImail reflects changes

## Benefits

- ✅ **Bidirectional sync**: Changes from server are reflected in TUImail
- ✅ **Reliable read marking**: Read status is properly set on server
- ✅ **Consistent state**: Database and server stay synchronized
- ✅ **Background updates**: Flag changes are synced automatically
- ✅ **Error resilience**: Retry logic for failed operations

## Files to Modify

- `src/email.rs`: Add flag sync functions
- `src/app.rs`: Integrate flag sync into refresh
- `src/database.rs`: Ensure flag update functions work correctly

This fix will ensure that read/unread status is properly synchronized between TUImail and your email server.
