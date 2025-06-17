# Email Address Parsing Debug Guide

## Problem
Email addresses are showing as "Unknown" in the email list instead of displaying the actual sender names and addresses.

## Debug Logging Setup

The email client now logs detailed debug information to a file instead of mixing with the TUI interface.

### Method 1: Using the Debug Script
```bash
./debug_email.sh
```

This script will:
1. Clear any existing debug log
2. Run the email client with debug enabled
3. Show the debug log contents when you exit

### Method 2: Manual Debug
```bash
# Clear previous log
rm -f /tmp/email_client_debug.log

# Run with debug enabled
EMAIL_DEBUG=1 ./email_client

# In another terminal, watch the log in real-time:
tail -f /tmp/email_client_debug.log

# Or view the complete log after exiting:
cat /tmp/email_client_debug.log
```

## What the Debug Log Shows

The debug log will contain detailed information about:

### 1. **Email Client Initialization**
```
[2025-06-17 14:30:00] === Email Client Debug Log Started ===
[2025-06-17 14:30:00] Creating EmailClient for account: user@example.com
```

### 2. **Email Fetching Process**
```
[2025-06-17 14:30:01] fetch_emails called: folder='INBOX', limit=50
[2025-06-17 14:30:01] Loaded 0 cached emails
[2025-06-17 14:30:01] Fetching new emails from server using security: SSL
```

### 3. **IMAP Message Parsing**
```
[2025-06-17 14:30:02] Starting to parse 5 messages from folder 'INBOX'
[2025-06-17 14:30:02] Message 1: UID=123, body_length=2048, flags=["\\Seen"]
[2025-06-17 14:30:02] Message 1 body preview: Return-Path: <sender@example.com>...
```

### 4. **Header Extraction**
```
[2025-06-17 14:30:02] Email subject: 'Test Email'
[2025-06-17 14:30:02] Starting header extraction...
[2025-06-17 14:30:02] Header[1]: 'From' = 'John Doe <john@example.com>'
[2025-06-17 14:30:02] Found From header: 'John Doe <john@example.com>'
```

### 5. **Address Parsing**
```
[2025-06-17 14:30:02] Parsing email addresses from: 'John Doe <john@example.com>'
[2025-06-17 14:30:02] Processing address part: 'John Doe <john@example.com>'
[2025-06-17 14:30:02] Extracted: name='John Doe', email='john@example.com'
[2025-06-17 14:30:02] Parsed 1 addresses total
```

### 6. **Final Results**
```
[2025-06-17 14:30:02] Final email from addresses: 1 total
[2025-06-17 14:30:02]   Final From[0]: name=Some("John Doe"), address='john@example.com'
```

## Troubleshooting Based on Debug Output

### Issue 1: No Email Bodies
If you see:
```
Message 1: UID=123, body_length=0
```
**Problem**: IMAP isn't fetching email bodies
**Solution**: Check IMAP connection and permissions

### Issue 2: No Headers Found
If you see:
```
Processed 0 headers total
```
**Problem**: mail_parser isn't finding headers in the email body
**Solution**: Check email format and IMAP fetch command

### Issue 3: No From Header
If you see:
```
No From header found in headers map either
Available headers: ["Date", "Subject", "Message-ID"]
```
**Problem**: Email doesn't have a From header (unusual)
**Solution**: Check email source and server configuration

### Issue 4: Parsing Fails
If you see:
```
Unrecognized address format: 'malformed@email'
```
**Problem**: Email address format isn't supported
**Solution**: Add support for the specific format

## Expected Successful Output

A successful parse should look like:
```
[2025-06-17 14:30:02] Message 1: UID=123, body_length=2048, flags=["\\Seen"]
[2025-06-17 14:30:02] Message 1 parsed successfully by mail_parser
[2025-06-17 14:30:02] Email subject: 'Important Email'
[2025-06-17 14:30:02] Header[5]: 'From' = 'Jane Smith <jane@company.com>'
[2025-06-17 14:30:02] Found From header: 'Jane Smith <jane@company.com>'
[2025-06-17 14:30:02] Extracted: name='Jane Smith', email='jane@company.com'
[2025-06-17 14:30:02] Final From[0]: name=Some("Jane Smith"), address='jane@company.com'
```

## Next Steps

1. **Run the debug script**: `./debug_email.sh`
2. **Try to fetch some emails** in the client
3. **Exit the client** and check the debug output
4. **Share the debug log** to identify where the parsing is failing

The debug log will show exactly where the email address parsing is breaking down!
