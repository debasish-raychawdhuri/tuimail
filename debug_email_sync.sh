#!/bin/bash

echo "=== EMAIL SYNC DEBUGGING ==="
echo "Current time: $(date)"
echo

# Check if app is running
if pgrep -f tuimail > /dev/null; then
    echo "⚠️  TUImail is currently running. Please quit it first for accurate debugging."
    echo
fi

# Check database state
echo "=== DATABASE STATE ==="
DB_PATH="$HOME/.config/tuimail/emails.db"

if [ -f "$DB_PATH" ]; then
    echo "📊 Email counts per account/folder:"
    sqlite3 "$DB_PATH" "
    SELECT account_email, folder, COUNT(*) as email_count, MAX(uid) as max_uid, MAX(date_received) as latest_timestamp
    FROM emails 
    GROUP BY account_email, folder 
    ORDER BY account_email, folder;
    " | while IFS='|' read -r account folder count max_uid latest_ts; do
        latest_date=$(date -d "@$latest_ts" 2>/dev/null || echo "Invalid timestamp")
        echo "  📧 $account/$folder: $count emails, max UID: $max_uid, latest: $latest_date"
    done
    
    echo
    echo "📅 Most recent 3 emails:"
    sqlite3 "$DB_PATH" "
    SELECT account_email, folder, uid, subject, datetime(date_received, 'unixepoch') as received_time
    FROM emails 
    ORDER BY date_received DESC 
    LIMIT 3;
    " | while IFS='|' read -r account folder uid subject received_time; do
        echo "  📨 $account/$folder UID:$uid - $subject ($received_time)"
    done
else
    echo "❌ Database not found at $DB_PATH"
fi

echo
echo "=== SYNC TRACKER STATE ==="
echo "🔄 Global sync timestamps are stored in memory and reset when app restarts"
echo "   This might explain why new emails aren't being detected after app restart"

echo
echo "=== RECOMMENDATIONS ==="
echo "1. 🔧 Run: EMAIL_DEBUG=1 tuimail"
echo "2. 📋 Press 'r' to manually refresh"
echo "3. 📝 Check /tmp/tuimail_debug.log for sync errors"
echo "4. 🔍 Look for 'No new messages to fetch' vs actual server state"

echo
echo "=== QUICK FIX ==="
echo "If emails are missing, try:"
echo "1. Quit tuimail completely"
echo "2. Delete sync state: rm -f ~/.config/tuimail/sync_state.json 2>/dev/null"
echo "3. Restart tuimail - it will do a full sync"
