#!/bin/bash

# Test script to verify the refresh fix

echo "Testing TUImail Refresh Fix"
echo "=========================="
echo

# Check if config exists
CONFIG_FILE="$HOME/.config/tuimail/config.json"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "❌ No configuration found at $CONFIG_FILE"
    echo "Please run 'tuimail add-account' first to set up your email accounts."
    exit 1
fi

echo "✅ Configuration found"

# Build the application
echo "Building TUImail..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build successful"

# Check database before refresh
echo
echo "Checking database state before refresh..."

# Get the first account email from config
ACCOUNT_EMAIL=$(jq -r '.accounts[0].email' "$CONFIG_FILE" 2>/dev/null)
if [ "$ACCOUNT_EMAIL" = "null" ] || [ -z "$ACCOUNT_EMAIL" ]; then
    echo "❌ Could not extract account email from config"
    exit 1
fi

echo "Account: $ACCOUNT_EMAIL"

# Create database path
DB_DIR="$HOME/.cache/tuimail/$(echo "$ACCOUNT_EMAIL" | sed 's/@/_at_/g' | sed 's/\./_/g')"
DB_FILE="$DB_DIR/emails.db"

echo "Database path: $DB_FILE"

if [ -f "$DB_FILE" ]; then
    EMAIL_COUNT_BEFORE=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM emails WHERE account_email = '$ACCOUNT_EMAIL' AND folder = 'INBOX';" 2>/dev/null || echo "0")
    echo "Emails in database before refresh: $EMAIL_COUNT_BEFORE"
    
    # Get the latest email timestamp
    LATEST_TIMESTAMP=$(sqlite3 "$DB_FILE" "SELECT MAX(date_received) FROM emails WHERE account_email = '$ACCOUNT_EMAIL' AND folder = 'INBOX';" 2>/dev/null || echo "0")
    echo "Latest email timestamp before refresh: $LATEST_TIMESTAMP"
else
    echo "Database does not exist yet"
    EMAIL_COUNT_BEFORE=0
    LATEST_TIMESTAMP=0
fi

echo
echo "The fix changes the refresh functionality to:"
echo "1. Actually connect to IMAP server when pressing 'r'"
echo "2. Fetch new emails from the server"
echo "3. Update the local database with synced emails"
echo "4. Show synced emails in the UI"
echo
echo "Previous behavior: Only loaded from database (no IMAP sync)"
echo "New behavior: Syncs with IMAP server then loads fresh data"
echo
echo "To test:"
echo "1. Run: ./target/release/tuimail"
echo "2. Press 'r' to refresh"
echo "3. Check if recent emails appear"
echo "4. Look for 'Starting IMAP sync' messages in debug log"
echo
echo "Debug logging:"
echo "- Set EMAIL_DEBUG=1 to enable debug logging"
echo "- Debug logs go to /tmp/tuimail_debug.log"
echo
echo "Example: EMAIL_DEBUG=1 ./target/release/tuimail"

echo
echo "Fix Summary:"
echo "============"
echo "✅ Modified load_emails_for_account_folder() to call EmailClient.fetch_emails()"
echo "✅ This triggers actual IMAP sync instead of just database read"
echo "✅ Fallback to database if IMAP sync fails"
echo "✅ Better error handling and user feedback"
echo
echo "The refresh (r key) should now sync with your email server!"
