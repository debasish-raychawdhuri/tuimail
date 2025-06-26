#!/bin/bash

echo "🧪 Testing Read Status Operations"
echo "================================="

# Build the project first
echo "🔨 Building project..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful"

# Function to check pending operations
check_operations() {
    echo "📋 Checking pending operations..."
    python3 -c "
import sqlite3
import os

db_path = os.path.expanduser('~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db')
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

cursor.execute('SELECT COUNT(*) FROM email_operations WHERE processed = FALSE')
count = cursor.fetchone()[0]
print(f'Pending operations: {count}')

if count > 0:
    cursor.execute('''
        SELECT id, operation_type, email_uid, folder, created_at 
        FROM email_operations 
        WHERE processed = FALSE 
        ORDER BY created_at DESC
    ''')
    for row in cursor.fetchall():
        op_id, op_type, uid, folder, created = row
        print(f'  Operation {op_id}: {op_type} UID {uid} in {folder}')

conn.close()
"
}

# Function to check email read status
check_email_status() {
    local uid=$1
    echo "📧 Checking status of email UID $uid..."
    python3 -c "
import sqlite3
import os

db_path = os.path.expanduser('~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db')
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

cursor.execute('SELECT seen, subject FROM emails WHERE uid = ? LIMIT 1', ($uid,))
result = cursor.fetchone()
if result:
    seen, subject = result
    status = 'READ' if seen else 'UNREAD'
    print(f'  UID $uid: {status} - {subject[:50]}...')
else:
    print(f'  UID $uid: Not found')

conn.close()
"
}

echo ""
echo "🔍 Initial state check..."
check_operations

# Find an unread email to test with
echo ""
echo "🔍 Finding an unread email to test..."
UNREAD_UID=$(python3 -c "
import sqlite3
import os

db_path = os.path.expanduser('~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db')
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

cursor.execute('SELECT uid FROM emails WHERE seen = 0 ORDER BY date_received DESC LIMIT 1')
result = cursor.fetchone()
if result:
    print(result[0])
else:
    print('0')

conn.close()
")

if [ "$UNREAD_UID" = "0" ]; then
    echo "❌ No unread emails found for testing"
    exit 1
fi

echo "📧 Found unread email UID: $UNREAD_UID"
check_email_status $UNREAD_UID

echo ""
echo "🚀 Starting TUImail with debug logging..."
echo "   Instructions:"
echo "   1. Navigate to the unread email (UID $UNREAD_UID)"
echo "   2. Press Enter to open it (this should mark it as read)"
echo "   3. Press Esc to go back"
echo "   4. Press 'q' to quit"
echo ""
echo "Press Enter to continue..."
read

# Start TUImail with debug logging
EMAIL_DEBUG=1 timeout 60s ./target/release/tuimail

echo ""
echo "🔍 Post-test analysis..."

echo ""
echo "📋 Checking if operation was queued..."
check_operations

echo ""
echo "📧 Checking email status after opening..."
check_email_status $UNREAD_UID

echo ""
echo "📊 Waiting 5 seconds for background processing..."
sleep 5

echo ""
echo "📋 Checking operations after background processing..."
check_operations

echo ""
echo "📧 Final email status check..."
check_email_status $UNREAD_UID

echo ""
echo "🔍 Debug log analysis..."
if [ -f /tmp/tuimail_debug.log ]; then
    echo "📝 Recent debug log entries:"
    tail -20 /tmp/tuimail_debug.log | grep -E "(mark_read|operation|STORE|UID)"
else
    echo "❌ Debug log not found"
fi

echo ""
echo "✅ Test completed!"
