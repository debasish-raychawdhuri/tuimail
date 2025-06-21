#!/bin/bash

echo "🧪 Testing draychawdhuri account sync after metadata reset"
echo "========================================================="

# Check initial state
echo "📊 Initial state:"
./test_email_count.sh

echo
echo "🔄 Starting sync test..."
echo "This will run TUImail in debug mode and attempt to sync emails."
echo "The sync should now fetch ALL emails from the server (2249+)."
echo

# Run with timeout to avoid hanging
timeout 300s bash -c '
    echo "Starting TUImail with debug logging..."
    EMAIL_DEBUG=1 ./target/release/tuimail &
    TUIMAIL_PID=$!
    
    # Wait a bit for startup
    sleep 10
    
    # Kill TUImail after some time to check progress
    echo "Stopping TUImail to check progress..."
    kill $TUIMAIL_PID 2>/dev/null
    wait $TUIMAIL_PID 2>/dev/null
'

echo
echo "📊 Post-sync state:"
./test_email_count.sh

echo
echo "📋 Recent debug log entries:"
tail -20 /tmp/tuimail_debug.log | grep -E "(Folder.*has.*total|Initial sync|fetched.*messages|sync complete)"
