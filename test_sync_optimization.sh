#!/bin/bash

# Test script to verify the sync optimization is working

echo "🚀 Testing Email Sync Optimization"
echo "=================================="

# Enable debug mode
export EMAIL_DEBUG=1

# Clear debug log
> /tmp/tuimail_debug.log

echo "📧 Starting TUImail in background..."
timeout 30s ./target/debug/tuimail &
TUIMAIL_PID=$!

# Wait a bit for startup
sleep 5

echo "📊 Monitoring debug log for sync optimization messages..."
echo "Looking for 'Sync tracker indicates' and 'No new emails detected' messages..."

# Monitor the debug log for optimization messages
timeout 20s tail -f /tmp/tuimail_debug.log | while read line; do
    if [[ "$line" == *"Sync tracker indicates"* ]]; then
        echo "✅ FOUND: $line"
    elif [[ "$line" == *"No new emails detected"* ]]; then
        echo "✅ OPTIMIZATION WORKING: $line"
    elif [[ "$line" == *"Updated sync tracker timestamp"* ]]; then
        echo "✅ SYNC TRACKER UPDATE: $line"
    elif [[ "$line" == *"refresh_emails_from_database"* ]]; then
        echo "📝 Database refresh: $line"
    fi
done &

MONITOR_PID=$!

# Wait for the test
sleep 25

# Clean up
kill $TUIMAIL_PID 2>/dev/null
kill $MONITOR_PID 2>/dev/null

echo ""
echo "🔍 Analysis of debug log:"
echo "========================"

# Count optimization hits vs database queries
OPTIMIZATION_HITS=$(grep -c "No new emails detected" /tmp/tuimail_debug.log 2>/dev/null || echo "0")
DATABASE_QUERIES=$(grep -c "refresh_emails_from_database" /tmp/tuimail_debug.log 2>/dev/null || echo "0")
SYNC_UPDATES=$(grep -c "Updated sync tracker timestamp" /tmp/tuimail_debug.log 2>/dev/null || echo "0")

echo "📈 Optimization hits (skipped database queries): $OPTIMIZATION_HITS"
echo "🗄️  Database queries performed: $DATABASE_QUERIES"
echo "🔄 Sync tracker updates: $SYNC_UPDATES"

if [ "$OPTIMIZATION_HITS" -gt 0 ]; then
    echo "✅ SUCCESS: Sync optimization is working!"
    echo "   The UI is efficiently skipping unnecessary database queries."
else
    echo "⚠️  WARNING: No optimization hits detected."
    echo "   This might be normal if there are actually new emails."
fi

echo ""
echo "📋 Recent debug log entries:"
echo "============================"
tail -10 /tmp/tuimail_debug.log 2>/dev/null || echo "No debug log found"

echo ""
echo "🎯 Test completed!"
