#!/bin/bash

echo "🗂️  Testing File Browser Functionality"
echo "====================================="

# Enable debug mode
export EMAIL_DEBUG=1

# Clear debug log
> /tmp/tuimail_debug.log

echo "📧 Starting TUImail..."
echo "Instructions:"
echo "1. Navigate to an email with attachments"
echo "2. Press Tab to focus on attachments"
echo "3. Press 's' to save an attachment (should open file browser)"
echo "4. Or go to compose mode (press 'c')"
echo "5. Press Ctrl+A to add attachment (should open file browser)"
echo "6. Or press Ctrl+T to test file browser directly"
echo ""
echo "Expected behavior:"
echo "- File browser should appear as a dialog"
echo "- You should be able to navigate with arrow keys"
echo "- Press Esc to cancel, Enter to select"
echo ""
echo "Press Ctrl+C to exit when done testing"
echo ""

# Start TUImail
./target/debug/tuimail

echo ""
echo "🔍 Checking debug log for file browser activity..."

if [ -f /tmp/tuimail_debug.log ]; then
    echo "📋 File browser related log entries:"
    grep -i "file.browser\|test_file_browser\|file_browser_mode" /tmp/tuimail_debug.log | tail -10
    
    echo ""
    echo "📋 Input handling log entries:"
    grep -i "Input received\|file browser input" /tmp/tuimail_debug.log | tail -5
else
    echo "❌ No debug log found"
fi

echo ""
echo "🎯 Test completed!"
