#!/usr/bin/env python3

import sqlite3
import os
import time
import subprocess
import signal
import sys

def get_email_count(db_path):
    """Get current email count from database"""
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT COUNT(*) FROM emails WHERE folder = 'INBOX'")
        count = cursor.fetchone()[0]
        conn.close()
        return count
    except:
        return 0

def get_metadata(db_path):
    """Get metadata from database"""
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT last_uid, total_messages FROM folder_metadata WHERE folder = 'INBOX'")
        result = cursor.fetchone()
        conn.close()
        return result if result else (0, 0)
    except:
        return (0, 0)

def main():
    print("🔍 Verifying draychawdhuri account sync fix")
    print("=" * 50)
    
    db_path = os.path.expanduser("~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db")
    
    if not os.path.exists(db_path):
        print(f"❌ Database not found: {db_path}")
        return
    
    print("📊 Current state:")
    initial_count = get_email_count(db_path)
    initial_metadata = get_metadata(db_path)
    print(f"   Emails in database: {initial_count}")
    print(f"   Metadata: last_uid={initial_metadata[0]}, total_messages={initial_metadata[1]}")
    
    if initial_metadata[0] != 0 or initial_metadata[1] != 0:
        print("❌ Metadata not reset properly!")
        return
    
    print("\n🚀 Starting TUImail to trigger sync...")
    print("   This should now perform a FULL sync of all 2249+ emails")
    
    # Start TUImail in background
    env = os.environ.copy()
    env['EMAIL_DEBUG'] = '1'
    
    try:
        process = subprocess.Popen(
            ['./target/release/tuimail'],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            preexec_fn=os.setsid
        )
        
        print("   TUImail started, monitoring progress...")
        
        # Monitor for 60 seconds
        start_time = time.time()
        last_count = initial_count
        
        while time.time() - start_time < 60:
            current_count = get_email_count(db_path)
            current_metadata = get_metadata(db_path)
            
            if current_count != last_count:
                print(f"   📈 Progress: {current_count} emails (last_uid: {current_metadata[0]})")
                last_count = current_count
            
            time.sleep(2)
        
        # Stop TUImail
        os.killpg(os.getpgid(process.pid), signal.SIGTERM)
        process.wait(timeout=5)
        
    except Exception as e:
        print(f"   ⚠️  Error running TUImail: {e}")
    
    print("\n📊 Final state:")
    final_count = get_email_count(db_path)
    final_metadata = get_metadata(db_path)
    print(f"   Emails in database: {final_count}")
    print(f"   Metadata: last_uid={final_metadata[0]}, total_messages={final_metadata[1]}")
    
    print("\n📋 Analysis:")
    if final_count > initial_count:
        print(f"   ✅ SUCCESS: Synced {final_count - initial_count} new emails!")
        if final_count >= 2000:
            print("   🎉 Full sync appears to be working - most/all emails synced!")
        else:
            print("   ⏳ Partial sync - may need more time to complete")
    else:
        print("   ❌ No new emails synced - there may be an issue")
    
    print(f"\n💡 Expected: ~2249 emails total")
    print(f"   Current:  {final_count} emails")
    print(f"   Progress: {(final_count/2249)*100:.1f}%")

if __name__ == "__main__":
    main()
