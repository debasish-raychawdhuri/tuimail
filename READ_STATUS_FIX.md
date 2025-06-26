# Read Status Sync Fix - TUImail

## Problem Solved

The read status was not being updated on the IMAP server when emails were opened in TUImail. This caused:

1. **Local-only updates**: Read status was updated in the UI but not on the server
2. **Inconsistent state**: Other email clients didn't see emails as read
3. **Queued operations not processed**: Operations were queued but never executed

## Root Cause

The background sync thread was only fetching new emails but **not processing queued operations** like `mark_read`, `mark_unread`, and `delete`. When you opened an email:

1. ✅ Operation was queued in `email_operations` table
2. ✅ Local UI was updated immediately for responsiveness  
3. ❌ **Background thread never processed the queued operation**
4. ❌ **IMAP server was never updated**

## Solution Implemented

### 1. Enhanced Background Sync Thread

Modified the background sync thread in `src/app.rs` to process queued operations **before** fetching new emails:

```rust
// Process queued operations first
match database.get_pending_operations() {
    Ok(operations) => {
        for (op_id, account_email, operation_type, email_uid, folder, _) in operations {
            // Process mark_read, mark_unread, delete operations
            // Update IMAP server and local database
            // Mark operation as processed
        }
    }
}
```

### 2. Added Database Function

Added `delete_email()` function to `src/database.rs` for delete operations:

```rust
pub fn delete_email(&self, account_email: &str, folder: &str, uid: u32) -> Result<()> {
    self.conn.execute(
        "DELETE FROM emails WHERE account_email = ?1 AND folder = ?2 AND uid = ?3",
        params![account_email, folder, uid],
    )?;
    Ok(())
}
```

### 3. Operation Processing Logic

The background thread now processes three types of operations:

- **mark_read**: Calls `client.mark_as_read()` → Updates server → Updates database
- **mark_unread**: Calls `client.mark_as_unread()` → Updates server → Updates database  
- **delete**: Calls `client.delete_email()` → Updates server → Removes from database

## How It Works Now

### When You Open an Email:

1. **Immediate UI Update**: Email appears as read instantly (responsive UI)
2. **Queue Operation**: `mark_read` operation queued in database
3. **Background Processing**: Within 30 seconds, background thread:
   - Fetches queued operations
   - Connects to IMAP server
   - Executes `STORE +FLAGS (\Seen)` command
   - Updates database with server response
   - Marks operation as processed

### Background Sync Loop:

```
Every 30 seconds:
1. Process queued operations (mark_read, mark_unread, delete)
2. Fetch new emails from server
3. Update local database
4. Sleep for 30 seconds
```

## Testing the Fix

### 1. Run the Test Script:
```bash
./test_read_status_fix.sh
```

### 2. Check Pending Operations:
```bash
./check_pending_operations.py
```

### 3. Monitor Debug Logs:
```bash
export EMAIL_DEBUG=1
tail -f /tmp/tuimail_debug.log | grep -E "(Background sync|mark_read|Processing)"
```

### 4. Expected Log Messages:
```
Background sync thread started
Queued mark_read operation for email 123 in user@example.com/INBOX
Processing mark_read operation for email 123 in user@example.com/INBOX
Successfully processed mark_read operation for email 123
```

## Verification Steps

1. **Open an unread email** in TUImail (press Enter)
2. **Check debug logs** for queued and processed operations
3. **Wait up to 30 seconds** for background processing
4. **Check your webmail/other email client** - email should show as read
5. **Run operations checker** to verify no pending operations remain

## Benefits

- ✅ **Bidirectional sync**: Changes sync between TUImail and server
- ✅ **Reliable read marking**: Read status properly updated on IMAP server
- ✅ **Consistent state**: All email clients show same read/unread status
- ✅ **Background processing**: Operations processed automatically
- ✅ **Error resilience**: Failed operations remain queued for retry
- ✅ **Responsive UI**: Immediate local updates, server sync in background

## Files Modified

- `src/app.rs`: Enhanced background sync thread with operation processing
- `src/database.rs`: Added `delete_email()` function
- `test_read_status_fix.sh`: Test script for verification
- `check_pending_operations.py`: Database operations checker

The fix ensures that TUImail now properly synchronizes read/unread status with your IMAP server, making it consistent with other email clients.
