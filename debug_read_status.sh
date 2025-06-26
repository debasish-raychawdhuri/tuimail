#!/bin/bash

# Comprehensive debug script for read status issue

echo "🔍 DEBUG: Read Status Sync Issue"
echo "================================="
echo

# Enable debug logging
export EMAIL_DEBUG=1

# Clear previous debug log
> /tmp/tuimail_debug.log

echo "📋 Step 1: Check current pending operations"
./check_pending_operations.py
echo

echo "📋 Step 2: Starting TUImail with enhanced debug logging"
echo "   - All IMAP operations will be logged with ✅/❌ status"
echo "   - Connection tests will be performed on startup"
echo "   - Operation processing will be logged in detail"
echo

echo "📋 Step 3: Test procedure:"
echo "   1. Open an unread email (press Enter)"
echo "   2. Note the immediate UI change (should show as read)"
echo "   3. Wait 2-4 seconds for background processing"
echo "   4. Press 'r' to refresh"
echo "   5. Check if email stays read or reverts to unread"
echo

echo "📋 Step 4: Debug monitoring commands (run in separate terminals):"
echo "   Monitor all debug output:"
echo "   tail -f /tmp/tuimail_debug.log"
echo
echo "   Monitor just operation processing:"
echo "   tail -f /tmp/tuimail_debug.log | grep -E '(🔄|✅|❌|Quick)'"
echo
echo "   Monitor IMAP operations:"
echo "   tail -f /tmp/tuimail_debug.log | grep -E '(IMAP|mark_as_read)'"
echo

echo "📋 Step 5: After testing, check operations again:"
echo "   ./check_pending_operations.py"
echo

echo "🚀 Starting TUImail now..."
echo "   Watch the debug output carefully for any ❌ error messages"
echo "   Press Ctrl+C to exit when done"
echo

# Start TUImail
./target/release/tuimail
