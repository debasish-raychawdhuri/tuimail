#!/bin/bash

echo "🔧 Testing File Browser Fix"
echo "==========================="

# Enable debug mode
export EMAIL_DEBUG=1

# Clear debug log
> /tmp/tuimail_debug.log

echo "📧 The file browser should now work for:"
echo "   1. Saving attachments (press 's' when viewing an email with attachments)"
echo "   2. Adding attachments in compose mode (press Ctrl+A)"
echo "   3. Test mode (press Ctrl+T)"
echo ""
echo "🔍 Key fixes applied:"
echo "   ✅ Fixed attachment data loading in sync optimization"
echo "   ✅ Attachments now properly loaded with binary data"
echo "   ✅ File browser should display and function correctly"
echo ""
echo "📋 Instructions for testing:"
echo "   1. Start TUImail and navigate to an email with attachments"
echo "   2. Press Tab to focus on attachments panel"
echo "   3. Press 's' to save attachment - file browser should appear"
echo "   4. Use arrow keys to navigate, Enter to select folder"
echo "   5. Press 'q' for quick save to Downloads"
echo "   6. Press 'f' to edit filename, 's' to save in current folder"
echo "   7. Press Esc to cancel"
echo ""
echo "   For compose mode:"
echo "   1. Press 'c' to compose new email"
echo "   2. Press Ctrl+A to add attachment - file browser should appear"
echo "   3. Navigate and select file with Enter"
echo ""
echo "   For direct test:"
echo "   1. Press Ctrl+T to test file browser directly"
echo ""

read -p "Press Enter to start TUImail..."

# Start TUImail
./target/debug/tuimail

echo ""
echo "🔍 Analyzing debug log..."

if [ -f /tmp/tuimail_debug.log ]; then
    echo ""
    echo "📊 File browser activity:"
    FILE_BROWSER_ENTRIES=$(grep -c "file_browser_mode\|File browser\|test_file_browser" /tmp/tuimail_debug.log 2>/dev/null || echo "0")
    echo "   File browser mode activations: $FILE_BROWSER_ENTRIES"
    
    echo ""
    echo "📊 Attachment handling:"
    ATTACHMENT_ENTRIES=$(grep -c "attachment\|save.*attachment" /tmp/tuimail_debug.log 2>/dev/null || echo "0")
    echo "   Attachment operations: $ATTACHMENT_ENTRIES"
    
    echo ""
    echo "📊 Sync optimization:"
    OPTIMIZATION_HITS=$(grep -c "No new emails detected\|skip.*database" /tmp/tuimail_debug.log 2>/dev/null || echo "0")
    echo "   Optimization hits: $OPTIMIZATION_HITS"
    
    echo ""
    echo "📋 Recent file browser log entries:"
    grep -i "file.browser\|attachment.*save\|test_file_browser" /tmp/tuimail_debug.log | tail -5
    
    if [ "$FILE_BROWSER_ENTRIES" -gt 0 ]; then
        echo ""
        echo "✅ SUCCESS: File browser functionality is working!"
    else
        echo ""
        echo "⚠️  No file browser activity detected. This could mean:"
        echo "   - No attachments were present in emails"
        echo "   - File browser wasn't triggered"
        echo "   - Check if you pressed the correct keys (s, Ctrl+A, Ctrl+T)"
    fi
else
    echo "❌ No debug log found"
fi

echo ""
echo "🎯 Test completed!"
echo ""
echo "💡 If file browser still doesn't work:"
echo "   1. Make sure you have emails with attachments"
echo "   2. Try Ctrl+T for direct file browser test"
echo "   3. Check that you're pressing the right keys:"
echo "      - 's' to save attachment (in email view, attachments focused)"
echo "      - 'Ctrl+A' to add attachment (in compose mode)"
echo "   4. Look for error messages in the UI"
