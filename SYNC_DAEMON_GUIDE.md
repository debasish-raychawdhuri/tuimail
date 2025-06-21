# TUImail Sync Daemon - Implementation Guide

## Overview

The new TUImail architecture separates email synchronization from the UI through a dedicated background sync daemon. This solves the synchronization issues by:

1. **Persistent Background Sync**: Emails sync continuously, even when UI is closed
2. **Database-Driven Architecture**: Single source of truth with atomic operations
3. **Non-Blocking UI**: UI reads from database instantly, no IMAP blocking
4. **Reliable State Management**: Sync state persists across restarts

## Architecture Components

### 1. Sync Daemon (`tuimail-sync`)
- **Purpose**: Background service that continuously syncs emails
- **Operation**: Connects to IMAP servers, fetches emails, stores in database
- **Features**:
  - Multi-account support
  - Incremental sync (only new emails)
  - Operation queue processing (mark read, delete, etc.)
  - Error handling and retry logic
  - Configurable sync intervals

### 2. Enhanced Database Layer
- **Tables Added**:
  - `sync_state`: Tracks sync progress per account/folder
  - `email_operations`: Queue for user actions (mark read, delete, etc.)
  - `sync_stats`: Statistics and error tracking
- **Features**:
  - Thread-safe concurrent access
  - Atomic transactions
  - Efficient pagination
  - Sync state persistence

### 3. Modified UI Client
- **Changes**:
  - Reads emails from database (fast, non-blocking)
  - Queues operations instead of direct IMAP calls
  - Real-time updates via database polling
  - Responsive interface with instant startup

## Installation and Setup

### 1. Build the New Components

```bash
# Build both the UI client and sync daemon
cargo build --release

# This creates two binaries:
# - target/release/tuimail (UI client)
# - target/release/tuimail-sync (sync daemon)
```

### 2. Initialize Database Schema

The sync daemon automatically initializes the required database schema on first run.

### 3. Configure Accounts

Use the existing account setup:

```bash
./target/release/tuimail add-account
```

## Usage

### Option 1: Using the Helper Script

```bash
# Make the script executable
chmod +x run_sync_daemon.sh

# Run the interactive script
./run_sync_daemon.sh
```

The script provides options for:
- One-time sync
- Foreground daemon
- Background daemon
- Status checking
- Stopping daemon

### Option 2: Manual Commands

#### Run One-Time Sync
```bash
./target/release/tuimail-sync --once
```

#### Run Sync Daemon (Foreground)
```bash
./target/release/tuimail-sync
```

#### Run Sync Daemon (Background)
```bash
./target/release/tuimail-sync --daemon &
```

#### Configure Sync Interval
```bash
./target/release/tuimail-sync --interval 60  # Sync every 60 seconds
```

#### Use Custom Paths
```bash
./target/release/tuimail-sync \
  --config ~/.config/tuimail/config.json \
  --database ~/.cache/tuimail/emails.db \
  --interval 30
```

### Option 3: System Service (Linux)

Create a systemd service for automatic startup:

```bash
# Create service file
sudo tee /etc/systemd/system/tuimail-sync.service > /dev/null <<EOF
[Unit]
Description=TUImail Email Sync Daemon
After=network.target

[Service]
Type=simple
User=$USER
ExecStart=$HOME/rust/email_client/target/release/tuimail-sync
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Enable and start service
sudo systemctl enable tuimail-sync
sudo systemctl start tuimail-sync

# Check status
sudo systemctl status tuimail-sync
```

## How It Works

### 1. Background Synchronization

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   IMAP Server   │◄──►│   Sync Daemon   │◄──►│   SQLite DB     │
│   (Gmail, etc.) │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │
                                ▼
                       ┌─────────────────┐
                       │   TUI Client    │
                       │   (Read-Only)   │
                       └─────────────────┘
```

### 2. Sync Process Flow

1. **Daemon starts** → Loads configuration and initializes database
2. **For each account** → Connects to IMAP server
3. **For each folder** → Checks sync state, fetches new emails
4. **Incremental sync** → Only fetches emails newer than last UID
5. **Database update** → Saves emails atomically with metadata
6. **State update** → Records sync progress and statistics
7. **Operation processing** → Handles queued user actions
8. **Sleep and repeat** → Waits for next sync interval

### 3. User Action Flow

1. **User marks email as read** → UI queues operation in database
2. **UI updates immediately** → Shows email as read for responsiveness
3. **Sync daemon processes** → Applies operation to IMAP server
4. **Confirmation** → Operation marked as completed in database

## Benefits

### 1. **Reliability**
- ✅ **No lost emails**: Persistent sync state survives crashes
- ✅ **Atomic operations**: No partial sync states
- ✅ **Error recovery**: Failed operations can be retried
- ✅ **Consistent data**: Single source of truth in database

### 2. **Performance**
- ✅ **Instant startup**: UI loads immediately from database
- ✅ **Non-blocking**: No UI freezing during sync
- ✅ **Efficient sync**: Only fetches new emails
- ✅ **Scalable**: Handles large email volumes

### 3. **User Experience**
- ✅ **Always up-to-date**: Background sync keeps emails fresh
- ✅ **Offline reading**: Access emails without network
- ✅ **Responsive UI**: Immediate feedback on actions
- ✅ **Multi-device**: Sync continues regardless of UI usage

## Monitoring and Troubleshooting

### Check Sync Status

```bash
# Using the helper script
./run_sync_daemon.sh  # Choose option 4

# Or manually check database
sqlite3 ~/.cache/tuimail/emails.db "
SELECT 
    account_email,
    folder,
    last_uid_seen,
    datetime(last_sync_timestamp, 'unixepoch') as last_sync,
    sync_in_progress
FROM sync_state;
"
```

### View Pending Operations

```bash
sqlite3 ~/.cache/tuimail/emails.db "
SELECT 
    account_email,
    operation_type,
    email_uid,
    folder,
    datetime(created_at, 'unixepoch') as created,
    processed,
    error
FROM email_operations 
WHERE processed = 0;
"
```

### Check Email Counts

```bash
sqlite3 ~/.cache/tuimail/emails.db "
SELECT 
    account_email,
    folder,
    COUNT(*) as email_count
FROM emails 
GROUP BY account_email, folder;
"
```

### Debug Logs

The sync daemon logs to stdout/stderr. For background operation:

```bash
# Run with logging
./target/release/tuimail-sync 2>&1 | tee /tmp/tuimail-sync.log

# Or redirect to file
./target/release/tuimail-sync > /tmp/tuimail-sync.log 2>&1 &
```

## Migration from Old Architecture

### 1. Backup Existing Data

```bash
# Backup current database
cp ~/.cache/tuimail/emails.db ~/.cache/tuimail/emails.db.backup

# Backup configuration
cp ~/.config/tuimail/config.json ~/.config/tuimail/config.json.backup
```

### 2. Build New Version

```bash
cargo build --release
```

### 3. Initialize Sync Daemon

```bash
# Run once to initialize new schema
./target/release/tuimail-sync --once
```

### 4. Verify Migration

```bash
# Check that emails are still accessible
./target/release/tuimail

# Check sync state
./run_sync_daemon.sh  # Option 4
```

## Configuration Options

### Sync Daemon Configuration

The sync daemon accepts these command-line options:

- `--config PATH`: Configuration file path (default: `~/.config/tuimail/config.json`)
- `--database PATH`: Database file path (default: `~/.cache/tuimail/emails.db`)
- `--interval SECONDS`: Sync interval in seconds (default: 30)
- `--daemon`: Run as background daemon
- `--once`: Run sync once and exit

### Environment Variables

- `EMAIL_DEBUG=1`: Enable debug logging
- `TUIMAIL_CONFIG`: Override config file path
- `TUIMAIL_DATABASE`: Override database file path

## Advanced Usage

### Custom Sync Intervals per Account

While not yet implemented, the architecture supports per-account sync intervals:

```json
{
  "accounts": [
    {
      "name": "Work Email",
      "email": "work@company.com",
      "sync_interval": 15,
      // ... other settings
    },
    {
      "name": "Personal Email", 
      "email": "personal@gmail.com",
      "sync_interval": 60,
      // ... other settings
    }
  ]
}
```

### Selective Folder Sync

Configure which folders to sync:

```json
{
  "accounts": [
    {
      "name": "Gmail",
      "email": "user@gmail.com",
      "sync_folders": ["INBOX", "Sent", "Important"],
      // ... other settings
    }
  ]
}
```

## Troubleshooting

### Common Issues

1. **Sync daemon won't start**
   - Check configuration file exists
   - Verify account credentials
   - Check network connectivity

2. **Emails not syncing**
   - Check sync daemon is running
   - Verify IMAP server settings
   - Check for error messages in logs

3. **UI shows old emails**
   - Restart UI client
   - Check database file permissions
   - Verify sync daemon is updating database

4. **High CPU usage**
   - Increase sync interval
   - Check for sync errors causing retries
   - Monitor database size and vacuum if needed

### Getting Help

1. **Enable debug logging**: `EMAIL_DEBUG=1`
2. **Check sync status**: Use helper script option 4
3. **Review logs**: Check daemon output for errors
4. **Database inspection**: Use sqlite3 to examine data
5. **Reset if needed**: Stop daemon, backup and recreate database

This new architecture provides a robust, scalable solution for email synchronization that eliminates the blocking and consistency issues of the previous design.
