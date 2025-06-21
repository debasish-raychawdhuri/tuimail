#!/bin/bash

echo "📊 Monitoring draychawdhuri account sync progress..."
echo "=================================================="

DB_PATH="$HOME/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db"

while true; do
    if [ -f "$DB_PATH" ]; then
        EMAIL_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM emails WHERE folder = 'INBOX';" 2>/dev/null || echo "0")
        METADATA=$(sqlite3 "$DB_PATH" "SELECT last_uid, total_messages FROM folder_metadata WHERE folder = 'INBOX';" 2>/dev/null || echo "0|0")
        
        echo "[$(date '+%H:%M:%S')] Emails: $EMAIL_COUNT | Metadata: $METADATA"
    else
        echo "[$(date '+%H:%M:%S')] Database not found yet..."
    fi
    
    sleep 5
done
