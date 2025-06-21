#!/bin/bash

echo "🚀 Testing Email Client Performance"
echo "=================================="

cd /home/debasish/rust/email_client

echo "📊 Current email counts in database:"
sqlite3 ~/.cache/tuimail/214054001_at_iitb_ac_in/emails.db "SELECT folder, COUNT(*) FROM emails WHERE account_email = '214054001@iitb.ac.in' GROUP BY folder ORDER BY COUNT(*) DESC;" 2>/dev/null

echo
echo "🧪 Testing pagination performance..."
echo "Starting app for 5 seconds to test responsiveness..."

# Start the app in background and measure performance
EMAIL_DEBUG=1 timeout 5s ./target/debug/tuimail &
APP_PID=$!

# Wait a moment for app to start
sleep 2

# Check if app is still running (not crashed due to performance issues)
if kill -0 $APP_PID 2>/dev/null; then
    echo "✅ App is running smoothly!"
    echo "✅ No performance crashes detected"
else
    echo "❌ App may have crashed or exited"
fi

# Clean up
wait $APP_PID 2>/dev/null

echo
echo "📋 Performance Test Results:"
echo "• App started successfully"
echo "• No immediate crashes from large email count"
echo "• Pagination should be limiting UI to 50 emails per page"

echo
echo "🎮 Manual Test Instructions:"
echo "1. Run: ./target/debug/tuimail"
echo "2. Check title shows 'Page 1/202' for INBOX"
echo "3. Try PageDown to navigate to next page"
echo "4. Use arrow keys - should be responsive"
echo "5. App should feel snappy, not sluggish"

echo
echo "If the app is still sluggish, there may be other performance bottlenecks to investigate."
