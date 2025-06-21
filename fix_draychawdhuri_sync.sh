#!/bin/bash

echo "🔧 Fixing draychawdhuri account sync issue"
echo "=========================================="

# Path to the draychawdhuri database
DB_PATH="$HOME/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db"

if [ ! -f "$DB_PATH" ]; then
    echo "❌ Database not found at: $DB_PATH"
    exit 1
fi

echo "📊 Current state:"
echo "Emails in database:"
sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM emails WHERE folder = 'INBOX';"

echo "Current metadata:"
sqlite3 "$DB_PATH" "SELECT folder, last_uid, total_messages FROM folder_metadata WHERE folder = 'INBOX';"

echo
echo "🔄 Resetting metadata to force complete re-sync..."

# Reset the metadata to force a complete re-sync
sqlite3 "$DB_PATH" "UPDATE folder_metadata SET last_uid = 0, total_messages = 0 WHERE folder = 'INBOX';"

echo "✅ Metadata reset complete!"
echo
echo "📋 New metadata state:"
sqlite3 "$DB_PATH" "SELECT folder, last_uid, total_messages FROM folder_metadata WHERE folder = 'INBOX';"

echo
echo "🚀 Next steps:"
echo "1. Run: EMAIL_DEBUG=1 ./target/release/tuimail"
echo "2. Navigate to the draychawdhuri account"
echo "3. Press 'r' to refresh - this will trigger a complete re-sync"
echo "4. Wait for the sync to complete (may take a few minutes for 2000+ emails)"
echo "5. Check the count again with: ./test_email_count.sh"
echo
echo "💡 The system will now fetch ALL 2249+ emails from the server instead of just trying incremental sync."
