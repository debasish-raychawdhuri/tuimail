#!/usr/bin/env python3

import sqlite3
import os

def test_operation_queue():
    """Test if we can manually queue an operation"""
    
    db_path = os.path.expanduser("~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db")
    if not os.path.exists(db_path):
        print("❌ Database not found")
        return
    
    print("🧪 Testing Operation Queue")
    print("=" * 30)
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Find an unread email
    cursor.execute("SELECT uid, subject FROM emails WHERE seen = 0 LIMIT 1")
    result = cursor.fetchone()
    
    if not result:
        print("❌ No unread emails found")
        return
    
    uid, subject = result
    print(f"📧 Found unread email: UID {uid}")
    print(f"   Subject: {subject[:50]}...")
    
    # Check current operations
    cursor.execute("SELECT COUNT(*) FROM email_operations WHERE processed = FALSE")
    before_count = cursor.fetchone()[0]
    print(f"📋 Operations before: {before_count}")
    
    # Manually queue a mark_read operation
    account_email = "draychawdhuri@cse.iitb.ac.in"
    folder = "INBOX"
    
    cursor.execute("""
        INSERT INTO email_operations (account_email, operation_type, email_uid, folder, target_folder, created_at)
        VALUES (?, ?, ?, ?, ?, strftime('%s', 'now'))
    """, (account_email, "mark_read", uid, folder, None))
    
    conn.commit()
    
    # Check operations after
    cursor.execute("SELECT COUNT(*) FROM email_operations WHERE processed = FALSE")
    after_count = cursor.fetchone()[0]
    print(f"📋 Operations after: {after_count}")
    
    # Show the queued operation
    cursor.execute("""
        SELECT id, operation_type, email_uid, folder, created_at
        FROM email_operations 
        WHERE processed = FALSE 
        ORDER BY created_at DESC
        LIMIT 1
    """)
    
    op_result = cursor.fetchone()
    if op_result:
        op_id, op_type, op_uid, op_folder, created = op_result
        print(f"✅ Queued operation {op_id}: {op_type} UID {op_uid} in '{op_folder}'")
    
    conn.close()
    
    print("")
    print("🚀 Now run TUImail and check if the operation gets processed!")
    print("   The background thread should process this operation within 2 seconds.")

if __name__ == "__main__":
    test_operation_queue()
