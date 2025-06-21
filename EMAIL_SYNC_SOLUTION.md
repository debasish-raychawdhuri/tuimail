# TUImail Email Sync Issues - Complete Solution Guide

## 🔍 Problem Analysis

### Current State
- **Account 1**: 111 emails stored (UID range: 9956-10066)
- **Account 2**: 111 emails stored (UID range: 2138-2248)
- **Issue**: Only recent emails are synced, not complete email history
- **Secondary Issue**: Application hangs on startup

### Root Causes
1. **Limited Initial Sync**: The sync process may be fetching only recent emails
2. **Server Limitations**: Email providers might limit IMAP access
3. **Connection Issues**: App hanging suggests authentication/connection problems

## 🛠️ Solutions Applied

### 1. Enhanced Debug Logging
- Added detailed sync progress tracking
- Better error reporting for sync mismatches
- Verification of server vs client email counts

### 2. Sync Logic Improvements
- Improved initial sync to fetch ALL messages from server
- Better handling of UID ranges and metadata
- Enhanced force full sync functionality

### 3. Performance Optimizations
- Database query optimizations
- Better memory management for large email sets

## 📋 Step-by-Step Resolution

### Step 1: Diagnose Connection Issues

```bash
# Test basic connectivity
ping imap.gmail.com  # or your email provider's IMAP server

# Check if TUImail can start at all
timeout 10s ./target/release/tuimail --help
```

### Step 2: Reset and Re-sync

```bash
# Option A: Clear cache and force fresh sync
rm -rf ~/.cache/tuimail/
./target/release/tuimail add-account  # Re-add accounts

# Option B: Force full sync with existing setup
EMAIL_DEBUG=1 ./target/release/tuimail
# Press Shift+R to force full sync
```

### Step 3: Check Email Provider Limits

**Gmail Users:**
- Gmail IMAP may limit access to recent emails by default
- Check Gmail settings: Settings → Forwarding and POP/IMAP
- Ensure "Enable IMAP" is checked
- Consider using App Passwords instead of regular password

**Other Providers:**
- Check IMAP settings in your email provider's documentation
- Some providers limit IMAP to recent emails (last 30 days, etc.)
- Verify IMAP server settings are correct

### Step 4: Manual Verification

```bash
# Check what your email provider actually has
# Log into your email web interface and count total emails

# Compare with TUImail database
sqlite3 ~/.cache/tuimail/*/emails.db "SELECT COUNT(*) FROM emails;"
```

## 🔧 Advanced Troubleshooting

### If App Hangs on Startup

1. **Check Credentials**:
   ```bash
   # Remove and re-add accounts
   rm ~/.config/tuimail/config.json
   ./target/release/tuimail add-account
   ```

2. **Test Network Connectivity**:
   ```bash
   # Test IMAP connection manually
   telnet imap.gmail.com 993  # For Gmail
   # Should connect without hanging
   ```

3. **Check Firewall/Proxy**:
   - Ensure IMAP ports (993, 143) are not blocked
   - Check if corporate firewall is interfering

### If Sync is Limited

1. **Provider-Specific Solutions**:

   **Gmail**:
   - Enable "Less secure app access" (if using password)
   - Use App Passwords (recommended)
   - Check Gmail IMAP settings

   **Outlook/Hotmail**:
   - Verify IMAP is enabled in account settings
   - Use App Password for authentication

   **Corporate Email**:
   - Check with IT department for IMAP limitations
   - Some corporate servers limit IMAP access

2. **Force Complete Re-sync**:
   ```bash
   # Clear all cached data
   rm -rf ~/.cache/tuimail/
   
   # Start fresh
   EMAIL_DEBUG=1 ./target/release/tuimail
   ```

## 📊 Expected Results After Fix

### Before Fix:
- Only ~111 recent emails synced
- High UID ranges (missing older emails)
- Limited email history access

### After Fix:
- ALL emails from server should sync
- Complete UID range coverage
- Full email history accessible
- Better sync progress reporting

## 🚀 Testing the Fix

### Quick Test:
```bash
# 1. Check current state
sqlite3 ~/.cache/tuimail/*/emails.db "SELECT COUNT(*) FROM emails;"

# 2. Force full sync
EMAIL_DEBUG=1 ./target/release/tuimail
# Press Shift+R, wait for completion, press 'q'

# 3. Check results
sqlite3 ~/.cache/tuimail/*/emails.db "SELECT COUNT(*) FROM emails;"
grep -E "server reports|fetched.*messages" /tmp/tuimail_debug.log
```

### Comprehensive Test:
```bash
# Run the provided test script
./test_sync_manually.sh
```

## 🔍 Verification Checklist

- [ ] TUImail starts without hanging
- [ ] Email count matches your email provider's web interface
- [ ] Older emails are accessible (not just recent ones)
- [ ] Shift+R force sync works properly
- [ ] Debug logs show complete sync progress

## 📞 If Issues Persist

1. **Check Email Provider Documentation**:
   - Look for IMAP-specific limitations
   - Verify server settings and ports

2. **Provider-Specific Troubleshooting**:
   - Gmail: Check Google Account security settings
   - Outlook: Verify IMAP is enabled
   - Corporate: Contact IT support

3. **Alternative Solutions**:
   - Consider using OAuth2 authentication (if supported)
   - Check if provider offers unlimited IMAP access options
   - Consider archiving old emails if provider limits are unavoidable

## 📝 Summary

The email sync issues in TUImail are primarily due to:
1. Limited initial sync fetching only recent emails
2. Potential email provider IMAP limitations
3. Connection/authentication issues causing startup hangs

The fixes applied improve sync logic, add better debugging, and provide tools to diagnose and resolve these issues. The key is to identify whether the limitation is client-side (TUImail) or server-side (email provider).
