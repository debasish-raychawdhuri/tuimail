#!/usr/bin/env python3

import sqlite3
import os
import time

def monitor_operations():
    """Monitor email operations in real-time"""
    
    db_path = os.path.expanduser("~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db")
    if not os.path.exists(db_path):
        print("❌ Database not found")
        return
    
    print("🔍 Monitoring email operations...")
    print("=" * 40)
    
    last_count = 0
    
    try:
        while True:
            conn = sqlite3.connect(db_path)
            cursor = conn.cursor()
            
            # Check pending operations
            cursor.execute("SELECT COUNT(*) FROM email_operations WHERE processed = FALSE")
            current_count = cursor.fetchone()[0]
            
            if current_count != last_count:
                print(f"\n📊 Pending operations: {current_count}")
                
                if current_count > 0:
                    cursor.execute("""
                        SELECT id, operation_type, email_uid, folder, created_at
                        FROM email_operations 
                        WHERE processed = FALSE 
                        ORDER BY created_at DESC
                    """)
                    
                    for row in cursor.fetchall():
                        op_id, op_type, uid, folder, created = row
                        print(f"  🔄 Operation {op_id}: {op_type} UID {uid} in '{folder}'")
                
                last_count = current_count
            
            conn.close()
            time.sleep(1)
            
    except KeyboardInterrupt:
        print("\n\n✅ Monitoring stopped")

if __name__ == "__main__":
    monitor_operations()
