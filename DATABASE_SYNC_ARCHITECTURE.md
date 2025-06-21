# Database-Driven Email Sync Architecture

## Problem Analysis

The current TUImail architecture has synchronization issues:

1. **UI-Coupled Sync**: Email fetching is tightly coupled with the UI, causing:
   - Blocking UI during sync operations
   - Inconsistent email state between sessions
   - Lost sync progress when app closes

2. **Memory-Based State**: Email data is primarily held in memory:
   - No persistent sync state
   - Full re-sync required on each startup
   - Race conditions between UI and background sync

3. **Limited Background Sync**: IDLE-based sync only works when UI is active:
   - No sync when app is closed
   - Depends on server IDLE support
   - Can't handle multiple accounts efficiently

## Proposed Solution: Separated Sync Architecture

### Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Background    │    │    SQLite       │    │   TUI Client    │
│   Sync Daemon   │◄──►│   Database      │◄──►│   (Read-Only)   │
│   (Write-Only)  │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Key Components

#### 1. Background Sync Daemon (`sync_daemon.rs`)
- **Purpose**: Continuously sync emails from IMAP servers to database
- **Operation**: Runs independently of UI, can be a separate process
- **Responsibilities**:
  - Connect to IMAP servers for all accounts
  - Fetch new emails incrementally
  - Update email flags (read/unread, etc.)
  - Handle folder synchronization
  - Maintain sync state and metadata

#### 2. Enhanced Database Layer (`database.rs`)
- **Purpose**: Central data store with proper concurrency control
- **Features**:
  - Thread-safe read/write operations
  - Atomic transactions for consistency
  - Efficient indexing for fast queries
  - Sync state tracking per account/folder

#### 3. Read-Only UI Client (`app.rs` + `ui.rs`)
- **Purpose**: Display emails and handle user interactions
- **Operation**: Reads from database, sends commands to sync daemon
- **Responsibilities**:
  - Display emails from database
  - Handle user actions (compose, reply, delete)
  - Send sync commands to daemon
  - Real-time UI updates via database polling/notifications

## Implementation Plan

### Phase 1: Enhanced Database Schema

```sql
-- Add sync state tracking
CREATE TABLE sync_state (
    account_email TEXT NOT NULL,
    folder TEXT NOT NULL,
    last_sync_timestamp INTEGER NOT NULL,
    last_uid_seen INTEGER NOT NULL,
    sync_in_progress BOOLEAN DEFAULT FALSE,
    last_error TEXT,
    PRIMARY KEY(account_email, folder)
);

-- Add email operations queue
CREATE TABLE email_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_email TEXT NOT NULL,
    operation_type TEXT NOT NULL, -- 'mark_read', 'mark_unread', 'delete', 'move'
    email_uid INTEGER NOT NULL,
    folder TEXT NOT NULL,
    target_folder TEXT, -- for move operations
    created_at INTEGER NOT NULL,
    processed BOOLEAN DEFAULT FALSE,
    error TEXT
);

-- Add sync statistics
CREATE TABLE sync_stats (
    account_email TEXT PRIMARY KEY,
    total_emails INTEGER DEFAULT 0,
    last_full_sync INTEGER DEFAULT 0,
    sync_errors INTEGER DEFAULT 0,
    last_successful_sync INTEGER DEFAULT 0
);
```

### Phase 2: Background Sync Daemon

```rust
// src/sync_daemon.rs
pub struct SyncDaemon {
    database: Arc<EmailDatabase>,
    accounts: Vec<EmailAccount>,
    running: Arc<AtomicBool>,
}

impl SyncDaemon {
    pub async fn start(&self) -> Result<()> {
        while self.running.load(Ordering::Relaxed) {
            for account in &self.accounts {
                self.sync_account(account).await?;
            }
            
            // Process pending operations (mark read, delete, etc.)
            self.process_email_operations().await?;
            
            // Sleep before next sync cycle
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        Ok(())
    }
    
    async fn sync_account(&self, account: &EmailAccount) -> Result<()> {
        let folders = self.get_folders(account).await?;
        
        for folder in folders {
            self.sync_folder(account, &folder).await?;
        }
        Ok(())
    }
    
    async fn sync_folder(&self, account: &EmailAccount, folder: &str) -> Result<()> {
        // Get last sync state
        let sync_state = self.database.get_sync_state(&account.email, folder)?;
        
        // Connect to IMAP and fetch new emails
        let mut client = self.connect_imap(account).await?;
        let new_emails = client.fetch_emails_since_uid(folder, sync_state.last_uid_seen).await?;
        
        // Save to database atomically
        self.database.save_emails_batch(&account.email, folder, &new_emails)?;
        
        // Update sync state
        self.database.update_sync_state(&account.email, folder, &sync_state)?;
        
        Ok(())
    }
}
```

### Phase 3: Database Enhancements

```rust
// Enhanced database.rs
impl EmailDatabase {
    pub fn save_emails_batch(&self, account_email: &str, folder: &str, emails: &[Email]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        
        for email in emails {
            // Insert email with conflict resolution
            self.insert_or_update_email(&tx, account_email, folder, email)?;
        }
        
        // Update folder statistics
        self.update_folder_stats(&tx, account_email, folder)?;
        
        tx.commit()?;
        Ok(())
    }
    
    pub fn get_emails_paginated(&self, account_email: &str, folder: &str, 
                               offset: usize, limit: usize) -> Result<Vec<Email>> {
        // Efficient pagination with proper indexing
        let mut stmt = self.conn.prepare(
            "SELECT * FROM emails 
             WHERE account_email = ?1 AND folder = ?2 
             ORDER BY date_received DESC 
             LIMIT ?3 OFFSET ?4"
        )?;
        
        // ... implementation
    }
    
    pub fn queue_email_operation(&self, operation: EmailOperation) -> Result<()> {
        // Queue operations for background processing
        self.conn.execute(
            "INSERT INTO email_operations (account_email, operation_type, email_uid, folder, created_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))",
            params![operation.account_email, operation.operation_type, operation.email_uid, operation.folder]
        )?;
        Ok(())
    }
}
```

### Phase 4: UI Decoupling

```rust
// Modified app.rs
impl App {
    pub fn load_emails_from_database(&mut self) -> AppResult<()> {
        if let Some(account) = self.get_current_account() {
            let folder = &self.selected_folder;
            
            // Load emails from database (fast, non-blocking)
            let emails = self.database.get_emails_paginated(
                &account.email, 
                folder, 
                self.email_offset, 
                50
            )?;
            
            self.emails = emails;
            
            // Trigger background sync if needed
            self.request_sync_if_stale(&account.email, folder)?;
        }
        Ok(())
    }
    
    pub fn mark_email_as_read(&mut self, email_uid: u32) -> AppResult<()> {
        if let Some(account) = self.get_current_account() {
            // Queue operation for background processing
            let operation = EmailOperation {
                account_email: account.email.clone(),
                operation_type: "mark_read".to_string(),
                email_uid,
                folder: self.selected_folder.clone(),
                ..Default::default()
            };
            
            self.database.queue_email_operation(operation)?;
            
            // Update local state immediately for responsive UI
            if let Some(email) = self.emails.iter_mut().find(|e| e.id == email_uid.to_string()) {
                email.seen = true;
            }
        }
        Ok(())
    }
}
```

## Benefits of This Architecture

### 1. **Reliability**
- **Persistent sync state**: Survives app restarts
- **Atomic operations**: No partial sync states
- **Error recovery**: Failed syncs can be retried
- **Data consistency**: Single source of truth in database

### 2. **Performance**
- **Non-blocking UI**: Database reads are fast
- **Efficient sync**: Only fetch new emails
- **Background processing**: Sync continues when UI is closed
- **Pagination**: Handle large email volumes efficiently

### 3. **Scalability**
- **Multiple accounts**: Sync all accounts independently
- **Concurrent access**: Multiple UI instances can read safely
- **Resource management**: Control memory usage and network connections
- **Extensibility**: Easy to add new sync features

### 4. **User Experience**
- **Instant startup**: UI loads immediately from database
- **Real-time updates**: Background sync keeps data fresh
- **Offline capability**: Read emails without network connection
- **Responsive interface**: No blocking operations in UI

## Migration Strategy

### Step 1: Enhance Database Schema
- Add new tables for sync state and operations
- Migrate existing data to new schema
- Add proper indexes for performance

### Step 2: Implement Sync Daemon
- Create background sync service
- Test with single account first
- Add multi-account support
- Implement operation queue processing

### Step 3: Modify UI Layer
- Change UI to read from database only
- Implement operation queuing for user actions
- Add real-time update mechanism
- Remove direct IMAP calls from UI

### Step 4: Testing and Optimization
- Test with large email volumes
- Optimize database queries
- Add monitoring and logging
- Performance tuning

## Configuration Changes

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
    "refresh_interval_seconds": 5,
    "page_size": 50,
    "preload_pages": 2
  }
}
```

This architecture will solve the synchronization issues by:
1. **Separating concerns**: Sync daemon handles IMAP, UI handles display
2. **Persistent state**: Database maintains sync state across sessions
3. **Better performance**: Non-blocking UI with efficient database queries
4. **Reliability**: Atomic operations and proper error handling
5. **Scalability**: Handle multiple accounts and large email volumes

The result will be a much more reliable and responsive email client that keeps emails properly synchronized without blocking the user interface.
