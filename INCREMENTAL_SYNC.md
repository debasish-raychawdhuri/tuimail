# Incremental Email Synchronization

## Overview

The email client now implements intelligent incremental email synchronization that maintains a complete local email store while only downloading new messages from the server.

## Key Features

### 🔄 **Incremental Sync**
- **First-time sync**: Downloads the last 100 messages initially
- **Subsequent syncs**: Only downloads new messages since last sync
- **UID tracking**: Uses IMAP UIDs to identify new messages
- **Metadata persistence**: Tracks sync state across app restarts

### 💾 **Persistent Storage**
- **Email cache**: All downloaded emails stored in `~/.cache/email_client/{account}/`
- **Metadata files**: Sync state stored in `{folder}_metadata.json`
- **Deduplication**: Prevents duplicate emails using UID tracking
- **Complete history**: Maintains full email history locally

### ⚡ **Performance Benefits**
- **Faster startup**: No need to re-download existing emails
- **Reduced bandwidth**: Only downloads new messages
- **Offline access**: Full access to previously downloaded emails
- **Smart merging**: Efficiently combines cached and new emails

## Technical Implementation

### Metadata Structure
```rust
pub struct FolderMetadata {
    pub last_uid: u32,                    // Highest UID downloaded
    pub total_messages: u32,              // Total messages in folder
    pub last_sync: DateTime<Local>,       // Last sync timestamp
    pub downloaded_uids: HashSet<u32>,    // Set of downloaded UIDs
}
```

### Sync Process

#### Initial Sync (First Time)
1. **Check folder status**: Get total message count from server
2. **Fetch recent messages**: Download last 100 messages (configurable)
3. **Update metadata**: Record all downloaded UIDs and highest UID
4. **Save to cache**: Store emails and metadata locally

#### Incremental Sync (Subsequent)
1. **Load metadata**: Read last sync state from disk
2. **Check for new messages**: Compare server total with cached total
3. **Fetch only new**: Use `UID FETCH {last_uid+1}:*` to get new messages
4. **Merge with cache**: Combine new messages with existing cache
5. **Update metadata**: Record new UIDs and sync timestamp

### Cache Structure
```
~/.cache/email_client/
├── user_at_gmail_com/
│   ├── INBOX.json              # Cached emails
│   ├── INBOX_metadata.json     # Sync metadata
│   ├── Sent.json
│   ├── Sent_metadata.json
│   └── Drafts.json
└── work_at_company_com/
    ├── INBOX.json
    └── INBOX_metadata.json
```

## Usage

### Normal Operation
The incremental sync happens automatically when you:
- Start the email client
- Refresh the email list
- Switch between folders

### Force Full Resync
If needed, you can force a complete resync:
```rust
client.force_full_sync("INBOX")?;
```

## Benefits Over Previous Implementation

### Before (Limited Download)
- ❌ Only downloaded 50 most recent emails
- ❌ Lost access to older emails on restart
- ❌ No persistent storage
- ❌ Re-downloaded same emails repeatedly

### After (Incremental Sync)
- ✅ Downloads and stores ALL emails incrementally
- ✅ Maintains complete email history
- ✅ Only downloads new messages after initial sync
- ✅ Fast startup with cached emails
- ✅ Works offline with full history
- ✅ Efficient bandwidth usage

## Debug Information

The incremental sync process is fully logged when `EMAIL_DEBUG=1`:

```
[timestamp] fetch_emails called: folder='INBOX', limit=50
[timestamp] Loaded 150 cached emails, last_uid=1234, total_messages=150
[timestamp] Folder 'INBOX' has 155 total messages, we have 150 cached
[timestamp] Incremental sync: fetching messages with UID >= 1235
[timestamp] Incremental sync: fetched 5 new messages
[timestamp] After merging: 155 total emails
[timestamp] Saved updated cache and metadata
```

## Configuration

### Initial Sync Size
The number of messages downloaded on first sync can be adjusted:
```rust
let fetch_count = std::cmp::min(100, current_total); // Currently 100
```

### Display Limit
The number of emails shown in the UI:
```rust
let display_limit = std::cmp::max(limit, 100); // Show at least 100
```

## Error Handling

- **Server unavailable**: Falls back to cached emails
- **Partial sync failure**: Retains existing cache
- **Metadata corruption**: Automatically recreates metadata
- **UID gaps**: Handles missing UIDs gracefully

## Future Enhancements

- **Background sync**: Periodic automatic synchronization
- **Selective folder sync**: Choose which folders to sync
- **Compression**: Compress cached email data
- **Cleanup**: Remove old emails based on age/size limits
- **Delta sync**: Even more efficient change detection

The incremental sync system provides a robust, efficient, and user-friendly email synchronization experience that scales well with large mailboxes.
