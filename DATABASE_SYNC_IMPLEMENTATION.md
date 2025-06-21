# Database-Driven Email Sync Implementation

## Overview

This implementation transforms TUImail from a UI-coupled email client to a database-driven architecture with separated sync and UI processes. This solves synchronization issues and provides a much more reliable and responsive user experience.

## Architecture Components

### 1. Background Sync Daemon (`tuimail-sync`)

**Purpose**: Continuously synchronize emails from IMAP servers to the local database.

**Key Features**:
- Runs independently of the UI
- Can be started as a daemon process
- Handles multiple email accounts
- Processes email operations (mark read, delete, move)
- Maintains sync state and handles errors gracefully

**Usage**:
```bash
# Run sync once and exit
./tuimail-sync --once

# Run as background daemon
./tuimail-sync --daemon

# Custom sync interval (default: 30 seconds)
./tuimail-sync --interval 60

# Custom database location
./tuimail-sync --database ~/.cache/tuimail/emails.db
```

### 2. Enhanced Database Layer

**New Tables**:
- `sync_state`: Tracks sync progress per account/folder
- `email_operations`: Queue for email operations (mark read, delete, etc.)
- `sync_stats`: Statistics and error tracking

**Key Methods**:
- `get_emails_paginated()`: Efficient email retrieval with pagination
- `queue_email_operation()`: Queue operations for background processing
- `is_sync_stale()`: Check if data needs refreshing
- `get_pending_operations()`: Get queued operations for processing

### 3. Database-Only UI

**Changes**:
- UI never makes direct IMAP calls
- All email data loaded from database
- Email operations queued instead of executed immediately
- Periodic database polling for real-time updates

**Benefits**:
- Instant startup (no IMAP connection delays)
- Responsive interface (no blocking operations)
- Works offline (read cached emails)
- Consistent state across sessions

## Implementation Details

### Database Schema Enhancements

```sql
-- Sync state tracking
CREATE TABLE sync_state (
    account_email TEXT NOT NULL,
    folder TEXT NOT NULL,
    last_sync_timestamp INTEGER NOT NULL DEFAULT 0,
    last_uid_seen INTEGER NOT NULL DEFAULT 0,
    sync_in_progress BOOLEAN DEFAULT FALSE,
    last_error TEXT,
    PRIMARY KEY(account_email, folder)
);

-- Email operations queue
CREATE TABLE email_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_email TEXT NOT NULL,
    operation_type TEXT NOT NULL, -- 'mark_read', 'mark_unread', 'delete', 'move'
    email_uid INTEGER NOT NULL,
    folder TEXT NOT NULL,
    target_folder TEXT, -- for move operations
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    processed BOOLEAN DEFAULT FALSE,
    error TEXT
);

-- Sync statistics
CREATE TABLE sync_stats (
    account_email TEXT PRIMARY KEY,
    total_emails INTEGER DEFAULT 0,
    last_full_sync INTEGER DEFAULT 0,
    sync_errors INTEGER DEFAULT 0,
    last_successful_sync INTEGER DEFAULT 0
);
```

### Sync Daemon Flow

1. **Initialization**:
   - Load configuration
   - Initialize database with sync schema
   - Create IMAP clients for each account

2. **Sync Loop**:
   - For each account:
     - Get list of folders
     - For each folder:
       - Check sync state
       - Fetch new emails since last UID
       - Save emails to database atomically
       - Update sync state
   - Process pending email operations
   - Sleep for configured interval

3. **Operation Processing**:
   - Get pending operations from queue
   - Execute IMAP operations (mark read, delete, etc.)
   - Update database immediately
   - Mark operations as processed

### UI Polling Mechanism

The UI polls the database every 2 seconds to check for:
- New emails (count changes)
- Flag updates (read/unread status changes)
- Deleted emails

```rust
// In main event loop
if last_db_poll.elapsed() >= DB_POLL_INTERVAL {
    app.refresh_emails_from_database()?;
    last_db_poll = std::time::Instant::now();
}
```

### Email Operation Queuing

When user performs actions (mark as read, delete), they are:
1. Queued in the database
2. Applied immediately to local UI state (for responsiveness)
3. Processed by sync daemon in background
4. Synchronized with IMAP server

```rust
// Queue operation
self.database.queue_email_operation(
    account_email,
    "mark_read",
    email_uid,
    folder,
    None
)?;

// Update local state immediately
email.seen = true;
```

## Benefits of This Architecture

### 1. Reliability
- **Persistent sync state**: Survives application restarts
- **Atomic operations**: No partial sync states
- **Error recovery**: Failed syncs can be retried
- **Data consistency**: Single source of truth in database

### 2. Performance
- **Non-blocking UI**: Database reads are fast (< 1ms)
- **Efficient sync**: Only fetch new emails incrementally
- **Background processing**: Sync continues when UI is closed
- **Pagination**: Handle large email volumes efficiently

### 3. User Experience
- **Instant startup**: UI loads immediately from database
- **Real-time updates**: Background sync keeps data fresh
- **Offline capability**: Read emails without network connection
- **Responsive interface**: No blocking operations in UI

### 4. Scalability
- **Multiple accounts**: Sync all accounts independently
- **Concurrent access**: Multiple UI instances can read safely
- **Resource management**: Control memory usage and network connections
- **Extensibility**: Easy to add new sync features

## Configuration

The sync daemon can be configured via command-line arguments or by extending the configuration file:

```json
{
  "sync_daemon": {
    "enabled": true,
    "sync_interval_seconds": 30,
    "max_concurrent_accounts": 5,
    "batch_size": 100
  },
  "database": {
    "path": "~/.cache/tuimail/emails.db",
    "vacuum_interval_hours": 24,
    "max_size_mb": 1000
  },
  "ui": {
    "refresh_interval_seconds": 2,
    "page_size": 50,
    "preload_pages": 2
  }
}
```

## Usage Examples

### Starting the Sync Daemon

```bash
# One-time sync (useful for testing)
tuimail-sync --once

# Background daemon with default settings
tuimail-sync --daemon

# Custom sync interval (60 seconds)
tuimail-sync --daemon --interval 60

# Verbose logging
EMAIL_DEBUG=1 tuimail-sync --daemon
```

### Using the UI

The UI usage remains the same, but now:
- Starts instantly (no IMAP connection wait)
- Shows cached emails immediately
- Updates in real-time as sync daemon fetches new emails
- Operations are queued and processed in background

### Monitoring

Check sync status:
```bash
# View database statistics
sqlite3 ~/.cache/tuimail/emails.db "
SELECT 
    account_email, 
    folder, 
    COUNT(*) as email_count,
    MAX(date_received) as latest_email
FROM emails 
GROUP BY account_email, folder;
"

# Check pending operations
sqlite3 ~/.cache/tuimail/emails.db "
SELECT * FROM email_operations WHERE processed = FALSE;
"

# View sync state
sqlite3 ~/.cache/tuimail/emails.db "
SELECT 
    account_email,
    folder,
    datetime(last_sync_timestamp, 'unixepoch') as last_sync,
    last_uid_seen,
    sync_in_progress
FROM sync_state;
"
```

## Troubleshooting

### Common Issues

1. **Sync daemon not starting**:
   - Check configuration file exists
   - Verify account credentials
   - Check debug logs: `tail -f /tmp/tuimail_debug.log`

2. **UI not updating**:
   - Verify sync daemon is running
   - Check database permissions
   - Ensure database polling is enabled

3. **Operations not processing**:
   - Check pending operations in database
   - Verify IMAP connection in sync daemon
   - Check for authentication errors

### Debug Mode

Enable debug logging:
```bash
EMAIL_DEBUG=1 tuimail-sync --daemon
EMAIL_DEBUG=1 tuimail
```

Logs are written to `/tmp/tuimail_debug.log`.

## Migration from Old Architecture

The new architecture is backward compatible. Existing installations will:
1. Continue to work with the old UI-coupled sync
2. Automatically create new database tables when sync daemon starts
3. Migrate existing email data to new schema
4. Benefit from improved performance immediately

To fully migrate:
1. Start the sync daemon: `tuimail-sync --daemon`
2. Use the UI normally - it will automatically use database-only mode
3. Old IMAP-coupled code paths will be bypassed

## Future Enhancements

This architecture enables several future improvements:
- **Push notifications**: Real-time email notifications
- **Multiple UI instances**: Run TUImail on multiple terminals
- **Web interface**: Add web UI that reads from same database
- **Mobile sync**: Sync with mobile email clients
- **Advanced search**: Full-text search across all emails
- **Email rules**: Server-side filtering and organization
- **Backup/restore**: Easy email backup and migration

## Conclusion

The database-driven architecture transforms TUImail from a simple IMAP client to a robust, scalable email management system. It solves the fundamental synchronization issues while providing a much better user experience and laying the foundation for future enhancements.

The separation of concerns (sync daemon for IMAP, database for storage, UI for display) makes the system more reliable, maintainable, and extensible.
