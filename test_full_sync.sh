#!/bin/bash

echo "=== Testing Full Email Sync ==="
echo

# Check current email count
echo "Current email count in database:"
sqlite3 ~/.cache/tuimail/emails.db "SELECT account_email, folder, COUNT(*) as count FROM emails GROUP BY account_email, folder ORDER BY account_email, folder;"
echo

# Run the app for a few seconds to let it sync
echo "Running TUImail for 10 seconds to test sync..."
timeout 10s ./target/debug/tuimail || echo "App finished"
echo

# Check email count after sync
echo "Email count after sync:"
sqlite3 ~/.cache/tuimail/emails.db "SELECT account_email, folder, COUNT(*) as count FROM emails GROUP BY account_email, folder ORDER BY account_email, folder;"
echo

# Check if we have more emails now
echo "=== Sync Test Complete ==="
