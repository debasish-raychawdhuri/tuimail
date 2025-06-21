# Dirty Flag Optimization for Email Sync

## Problem Statement

The email client was experiencing slow performance on accounts with large amounts of mail due to:

1. **Expensive UI polling**: Every 2 seconds, the UI was calling `check_for_new_emails()` which loaded 100 recent emails from the database to compare with current UI state
2. **Heavy background sync**: The background thread was doing full email fetching (`fetch_emails(&folder, 0)`) every 30 seconds, which is expensive for large mailboxes
3. **Unnecessary database queries**: Even when no new mail was available, the UI was still performing database queries

## Solution: Dirty Flag Pattern

We implemented a thread-safe dirty flag pattern using `Arc<AtomicBool>` to optimize the mail checking process:

### Architecture Changes

```
┌─────────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Background        │    │   Dirty Flag    │    │   UI Thread     │
│   Sync Thread       │    │ Arc<AtomicBool> │    │                 │
│                     │    │                 │    │                 │
│ • Lightweight UID   │───►│ Set TRUE when   │◄───│ • Check flag    │
│   checking          │    │ new mail found  │    │ • Only query DB │
│ • Only fetch new    │    │                 │    │   when flag set │
│   emails when found │    │ Reset to FALSE  │    │ • Reset flag    │
│                     │    │ after UI reads  │    │                 │
└─────────────────────┘    └─────────────────┘    └─────────────────┘
```

### Key Components

#### 1. Dirty Flag in App Struct
```rust
pub struct App {
    // ... existing fields ...
    
    // Dirty flag for efficient mail checking
    pub new_mail_dirty_flag: Arc<AtomicBool>,
}
```

#### 2. Optimized UI Mail Checking
```rust
pub fn check_for_new_emails(&mut self) {
    // Only check if the dirty flag is set (new mail detected by background thread)
    if !self.new_mail_dirty_flag.load(Ordering::Relaxed) {
        return; // No new mail, skip expensive database query
    }
    
    debug_log("Dirty flag set - checking for new emails from database");
    
    // Clear the dirty flag first to avoid race conditions
    self.new_mail_dirty_flag.store(false, Ordering::Relaxed);
    
    // ... proceed with database query only when needed ...
}
```

#### 3. Lightweight Background Sync
The background thread now performs lightweight checks instead of full syncs:

```rust
// Get the last known UID from database
match database.get_last_uid(&account.email, folder) {
    Ok(last_known_uid) => {
        // Lightweight check: just get the latest UID from server
        match client.get_latest_uid(folder) {
            Ok(latest_server_uid) => {
                if latest_server_uid > last_known_uid {
                    // Set dirty flag to trigger UI refresh
                    dirty_flag.store(true, Ordering::Relaxed);
                    
                    // Fetch only the new emails (incremental)
                    match client.fetch_emails_since_uid(folder, last_known_uid + 1) {
                        // ... save only new emails ...
                    }
                }
            }
        }
    }
}
```

### New Methods Added

#### EmailClient Methods
1. **`get_latest_uid(folder: &str) -> Result<u32, EmailError>`**
   - Lightweight IMAP operation to get the highest UID from server
   - Uses `SEARCH ALL` command which is much faster than fetching emails
   - Supports both SSL/StartTLS and plain connections

2. **`fetch_emails_since_uid(folder: &str, since_uid: u32) -> Result<Vec<Email>, EmailError>`**
   - Incremental email fetching using UID ranges
   - Only fetches emails with UID greater than the specified value
   - Significantly reduces network traffic and processing time

#### Database Methods
1. **`get_last_uid(account_email: &str, folder: &str) -> Result<u32>`**
   - Fast database query to get the highest UID stored locally
   - Uses `MAX(CAST(uid AS INTEGER))` for efficient lookup
   - Returns 0 if no emails exist for the account/folder

## Performance Benefits

### Before Optimization
- **UI Thread**: Database query every 2 seconds (100 emails loaded)
- **Background Thread**: Full email fetch every 30 seconds (all emails)
- **Network Traffic**: High (full mailbox sync)
- **Database Load**: High (frequent large queries)
- **CPU Usage**: High (processing all emails repeatedly)

### After Optimization
- **UI Thread**: Database query only when new mail detected
- **Background Thread**: Lightweight UID check + incremental fetch
- **Network Traffic**: Minimal (only new emails)
- **Database Load**: Minimal (single UID queries)
- **CPU Usage**: Minimal (process only new emails)

### Expected Performance Gains
- **90%+ reduction** in unnecessary database queries
- **95%+ reduction** in network traffic for accounts with no new mail
- **Instant UI responsiveness** when no new mail is available
- **Scalable performance** regardless of mailbox size

## Thread Safety

The implementation uses `Arc<AtomicBool>` which provides:
- **Lock-free operations**: No mutex contention between threads
- **Memory safety**: Rust's ownership system prevents data races
- **Atomic operations**: `load()` and `store()` are atomic and thread-safe
- **Relaxed ordering**: Sufficient for this use case (no strict ordering requirements)

## Race Condition Handling

The dirty flag is cleared **before** processing to avoid race conditions:

```rust
// Clear the dirty flag first to avoid race conditions
self.new_mail_dirty_flag.store(false, Ordering::Relaxed);

// Then process emails - if new mail arrives during processing,
// the flag will be set again and caught in the next cycle
```

## Configuration

The optimization works with existing configuration:
- **UI polling interval**: Still 2 seconds (but mostly no-ops now)
- **Background sync interval**: Still 30 seconds (but lightweight checks)
- **All existing functionality**: Preserved (compose, read, delete, etc.)

## Future Enhancements

This dirty flag pattern enables further optimizations:

1. **Multiple dirty flags**: Per-account or per-folder flags
2. **Priority queuing**: Urgent vs. normal mail notifications
3. **Push notifications**: Real-time alerts when flag is set
4. **Adaptive polling**: Adjust intervals based on mail frequency
5. **Background prefetching**: Preload mail content when flag is set

## Testing

To test the optimization:

1. **Build the project**: `cargo build --release`
2. **Run with debug logging**: `EMAIL_DEBUG=1 ./target/release/tuimail`
3. **Monitor logs**: Check `/tmp/tuimail_debug.log` for:
   - "Dirty flag set - checking for new emails from database"
   - "New mail detected for {account}: server UID {X} > local UID {Y}"
   - "No new mail for {account}: server UID {X} == local UID {Y}"

## Conclusion

The dirty flag optimization transforms the email client from a polling-heavy application to an event-driven system. This provides:

- **Better user experience**: Instant responsiveness
- **Lower resource usage**: CPU, memory, and network
- **Improved scalability**: Performance independent of mailbox size
- **Maintained functionality**: All existing features work unchanged

The optimization is particularly beneficial for:
- **Large mailboxes** (10,000+ emails)
- **Multiple accounts** (reduced per-account overhead)
- **Low-bandwidth connections** (minimal network usage)
- **Battery-powered devices** (reduced CPU usage)
