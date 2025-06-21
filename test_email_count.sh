#!/bin/bash

echo "=== Email Count Verification ==="
echo

echo "Account-specific databases (where emails are actually stored):"
echo "214054001@iitb.ac.in INBOX:"
sqlite3 ~/.cache/tuimail/214054001_at_iitb_ac_in/emails.db "SELECT COUNT(*) FROM emails WHERE folder = 'INBOX';" 2>/dev/null || echo "Database not found"

echo "draychawdhuri@cse.iitb.ac.in INBOX:"
sqlite3 ~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db "SELECT COUNT(*) FROM emails WHERE folder = 'INBOX';" 2>/dev/null || echo "Database not found"

echo
echo "Main shared database (should be empty or outdated):"
sqlite3 ~/.cache/tuimail/emails.db "SELECT account_email, folder, COUNT(*) FROM emails GROUP BY account_email, folder;" 2>/dev/null || echo "Database not found"

echo
echo "=== The Fix ==="
echo "✅ Background sync saves emails to account-specific databases"
echo "✅ App now reads from account-specific databases (same as background sync)"
echo "✅ This should resolve the missing emails issue"
