# Smart Sync Implementation - TUImail

## Problem Solved
The email client was not properly synchronizing read/unread status between the IMAP server and local database. This caused:

1. **Read status not updated to server**: When opening emails, read status wasn't always marked on server
2. **Server changes not reflected**: When other clients mark emails as read, TUImail didn't reflect these changes
3. **Inefficient sync**: Previous approach would try to sync all emails, which is impractical with tens of thousands of emails

## Solution: Smart Sync Strategy

### 1. Intelligent Sync Strategy Selection

The system now automatically chooses the best sync approach based on database state:

```rust
pub enum SyncStrategy {
    InitialSync { days_back: i64 },      // First time - sync last 30 days
    IncrementalSync { since_timestamp: i64 }, // Normal - sync since last email
    RecentSync { days_back: i64 },       // Fallback - sync last 7 days
}
```

**Strategy Selection Logic:**
- **No emails in database** → Initial sync (last 30 days)
- **Has emails with valid timestamps** → Incremental sync (since latest email)
- **Has emails but no timestamps** → Recent sync (last 7 days)
- **Database errors** → Fallback to initial sync

### 2. IMAP SINCE Query for Efficiency

Instead of fetching all emails, the system uses IMAP SEARCH with date criteria:

```rust
// Search for emails since a specific date
let search_result = session.search(&format!("SINCE {}", search_date))?;
```

This dramatically reduces the number of emails processed:
- **Before**: Potentially tens of thousands of emails
- **After**: Only emails since last sync (typically 0-100 emails)

### 3. Dual-Purpose Sync: New Emails + Flag Updates

For each email found by the SINCE query:

```rust
match database.email_exists(&self.account.email, folder, &uid_str) {
    Ok(true) => {
        // Email exists - check for flag changes
        let server_seen = server_flags.iter().any(|f| f == "\\Seen");
        database.update_email_seen_status(&self.account.email, folder, uid, server_seen)?;
    }
    Ok(false) => {
        // New email - parse and save
        let email = Email::from_parsed_email(&parsed, &uid_str, folder, flags)?;
        database.save_email(&email)?;
    }
}
```

This efficiently handles both:
- **New emails**: Parse and save to database
- **Flag changes**: Update read/unread status for existing emails

### 4. Batch Processing for Performance

```rust
// Process in batches to avoid server limits
for batch in search_vec.chunks(50) {
    let sequence_set = batch.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
    let messages = session.fetch(&sequence_set, "RFC822 FLAGS UID")?;
    // Process batch...
    std::thread::sleep(std::time::Duration::from_millis(100)); // Be nice to server
}
```

## Key Database Functions Added

### 1. Email Count Check
```rust
pub fn get_email_count(&self, account_email: &str, folder: &str) -> Result<i64>
```

### 2. Latest Email Timestamp
```rust
pub fn get_latest_email_timestamp(&self, account_email: &str, folder: &str) -> Result<Option<i64>>
```

### 3. Email Existence Check
```rust
pub fn email_exists(&self, account_email: &str, folder: &str, uid: &str) -> Result<bool>
```

### 4. Individual Email Save
```rust
pub fn save_email(&self, email: &Email) -> Result<()>
```

### 5. Flag Status Update
```rust
pub fn update_email_seen_status(&self, account_email: &str, folder: &str, uid: u32, seen: bool) -> Result<()>
```

## Integration Points

### App Refresh (src/app.rs)
```rust
// Use smart sync strategy instead of fetch_emails
match client.smart_sync(folder) {
    Ok(new_emails) => {
        // Load recent emails from database for UI display
        match account_database.get_recent_emails(&account_data.email, folder, 1000) {
            Ok(recent_emails) => {
                // Update UI with recent emails
            }
        }
    }
}
```

### Email Client (src/email.rs)
```rust
pub fn smart_sync(&self, folder: &str) -> Result<Vec<Email>, EmailError> {
    let strategy = self.determine_sync_strategy(folder)?;
    
    match strategy {
        SyncStrategy::InitialSync { days_back } => self.sync_recent_emails(folder, days_back),
        SyncStrategy::IncrementalSync { since_timestamp } => {
            let since_date = DateTime::<Utc>::from_timestamp(since_timestamp, 0);
            self.sync_emails_since_date(folder, since_date)
        }
        SyncStrategy::RecentSync { days_back } => self.sync_recent_emails(folder, days_back),
    }
}
```

## Performance Benefits

### Before (Inefficient)
- ❌ Tried to sync ALL emails in mailbox
- ❌ Would fail with tens of thousands of emails
- ❌ No flag synchronization
- ❌ Slow and resource-intensive

### After (Smart Sync)
- ✅ **Selective sync**: Only emails since last sync
- ✅ **Scalable**: Works with mailboxes of any size
- ✅ **Flag sync**: Bidirectional read status synchronization
- ✅ **Fast**: Typically syncs 0-100 emails instead of thousands
- ✅ **Intelligent**: Chooses best strategy based on database state
- ✅ **Robust**: Graceful fallback for edge cases

## Real-World Performance

### Typical Scenarios
1. **First run**: Syncs last 30 days (~100-500 emails)
2. **Daily refresh**: Syncs 0-20 new emails + flag updates
3. **After vacation**: Syncs emails since last use
4. **Large mailbox**: Only processes recent changes, not entire mailbox

### Server-Friendly
- Batch processing (50 emails at a time)
- 100ms delays between batches
- Uses efficient IMAP SEARCH instead of full folder scan

## Usage

The smart sync is automatically used when pressing 'r' to refresh emails. No user configuration needed - the system intelligently chooses the best approach based on current state.

## Testing

1. **Fresh install**: Should sync last 30 days
2. **Regular refresh**: Should only sync new emails since last refresh
3. **Flag changes**: Mark email as read in another client, refresh in TUImail - should reflect change
4. **Large mailbox**: Should remain fast regardless of total email count

This implementation solves the read status synchronization problem while being efficient and scalable for real-world use.
