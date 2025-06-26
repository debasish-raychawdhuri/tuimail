# Comprehensive Debug Guide - Read Status Issue

## Enhanced Debugging Features Added

### 1. Detailed IMAP Operation Logging
The `mark_as_read` function now logs every step:
- 🔄 Connection attempts (secure vs plain)
- ✅ Successful connections and folder selections  
- 📊 STORE command results
- ❌ Detailed error messages with full error context
- ⏳ Retry attempts with timing

### 2. Background Sync Thread Monitoring
Enhanced logging for the background sync process:
- 🔧 Email client initialization for each account
- 🔌 IMAP connection testing on startup
- 🔄 Operation processing with detailed status
- ⚡ Quick operation checks every 2 seconds

### 3. Operation Queue Tracking
Comprehensive operation lifecycle logging:
- Queue creation when emails are opened
- Background processing attempts
- Success/failure status with error details
- Database updates

## Testing Tools

### 1. Debug Script: `./debug_read_status.sh`
- Starts TUImail with full debug logging
- Provides monitoring commands
- Shows expected behavior

### 2. Operation Checker: `./check_pending_operations.py`
- Shows all queued operations
- Displays processing status
- Tracks operation history

### 3. Timing Monitor: `./verify_timing.py`
- Real-time operation processing monitoring
- Shows exact timing of operations
- Tracks new operations as they're created

### 4. Manual Test: `./test_mark_read.py`
- Manually inserts a mark_read operation
- Tests background processing without UI interaction
- Useful for isolating the sync thread behavior

## Debug Log Messages to Look For

### ✅ Success Indicators:
```
🔧 Initializing email client for user@example.com
✅ Credentials created for user@example.com
✅ IMAP connection test successful for user@example.com - found 5 folders
🔄 Background sync thread started
🔄 Processing mark_read operation for email 123 in user@example.com/INBOX
🔐 Using secure IMAP connection for user@example.com
✅ IMAP connection established for user@example.com
✅ Folder 'INBOX' selected. Mailbox info: exists=1234, recent=5
🔄 Attempting STORE command: UID 123 +FLAGS (\Seen)
✅ STORE command successful for UID 123
📊 STORE result: [...]
✅ IMAP mark_as_read succeeded for email 123
✅ Database update succeeded for email 123
✅ Successfully processed mark_read operation for email 123
```

### ❌ Error Indicators:
```
❌ Failed to create credentials for user@example.com: [error]
⚠️ IMAP connection test failed for user@example.com: [error]
❌ No email client found for account: user@example.com
❌ Invalid email ID '0', skipping mark as read
❌ Failed to establish IMAP connection: [error]
❌ Failed to select folder 'INBOX': [error]
❌ STORE command failed for UID 123: [error]
❌ IMAP mark_as_read FAILED for email 123: [error]
❌ Database update failed for email 123: [error]
❌ FAILED to process mark_read operation for email 123: [error]
```

## Step-by-Step Debugging Process

### Step 1: Start Debug Session
```bash
./debug_read_status.sh
```

### Step 2: Monitor Background Thread Startup
Look for these messages in the debug log:
```
🔧 Initializing email client for [account]
✅ Credentials created for [account]
✅ IMAP connection test successful for [account]
Background sync thread started
```

### Step 3: Test Email Opening
1. Open an unread email (press Enter)
2. Look for operation queuing:
   ```
   Queued mark_read operation for email [UID]
   ```

### Step 4: Monitor Processing (within 2-4 seconds)
Look for processing messages:
```
🔄 Processing mark_read operation for email [UID]
🔐 Using secure IMAP connection for [account]
✅ IMAP connection established
✅ Folder selected
🔄 Attempting STORE command
✅ STORE command successful
✅ Successfully processed mark_read operation
```

### Step 5: Verify Results
1. Press 'r' to refresh - email should stay read
2. Check operation status: `./check_pending_operations.py`
3. Check webmail - email should be marked as read

## Common Issues and Solutions

### Issue 1: No Background Thread Messages
**Symptoms**: No "Background sync thread started" message
**Cause**: Thread failed to start
**Solution**: Check account configuration and credentials

### Issue 2: Operations Not Being Processed
**Symptoms**: Operations queued but never processed
**Cause**: Background thread not running or IMAP connection issues
**Debug**: Look for connection test failures on startup

### Issue 3: IMAP Connection Failures
**Symptoms**: "❌ Failed to establish IMAP connection" messages
**Cause**: Network issues, wrong credentials, or server problems
**Solution**: Verify account settings and network connectivity

### Issue 4: STORE Command Failures
**Symptoms**: "❌ STORE command failed" messages
**Cause**: Invalid UID, folder issues, or server restrictions
**Debug**: Check UID validity and folder permissions

### Issue 5: Database Update Failures
**Symptoms**: IMAP succeeds but database update fails
**Cause**: Database corruption or permission issues
**Solution**: Check database file permissions and integrity

## Advanced Debugging

### Monitor Specific Operations:
```bash
tail -f /tmp/tuimail_debug.log | grep -E "(mark_read|STORE)"
```

### Monitor Connection Issues:
```bash
tail -f /tmp/tuimail_debug.log | grep -E "(❌|Failed|Error)"
```

### Monitor Success Path:
```bash
tail -f /tmp/tuimail_debug.log | grep -E "(✅|Successfully)"
```

### Check Operation Timing:
```bash
./verify_timing.py
```

This comprehensive debugging setup should reveal exactly where the read status sync is failing and provide detailed error information to fix the issue.
