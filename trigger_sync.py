#!/usr/bin/env python3

import subprocess
import time
import os
import signal
import sqlite3

def get_email_count():
    """Get current email count from database"""
    try:
        db_path = os.path.expanduser("~/.cache/tuimail/draychawdhuri_at_cse_iitb_ac_in/emails.db")
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT COUNT(*) FROM emails WHERE folder = 'INBOX'")
        count = cursor.fetchone()[0]
        conn.close()
        return count
    except:
        return 0

def main():
    print("🚀 Triggering draychawdhuri account sync")
    print("=" * 50)
    
    initial_count = get_email_count()
    print(f"📊 Initial email count: {initial_count}")
    
    # Start TUImail with debug logging
    env = os.environ.copy()
    env['EMAIL_DEBUG'] = '1'
    
    print("🔄 Starting TUImail...")
    process = subprocess.Popen(
        ['./target/release/tuimail'],
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    try:
        # Wait a bit for TUImail to start
        time.sleep(3)
        
        # Navigate to draychawdhuri account (assuming it's account 2)
        print("📧 Navigating to draychawdhuri account...")
        process.stdin.write('\t')  # Tab to switch accounts
        process.stdin.flush()
        time.sleep(1)
        
        # Press 'r' to refresh/sync
        print("🔄 Triggering sync with 'r' key...")
        process.stdin.write('r')
        process.stdin.flush()
        
        # Monitor progress for 2 minutes
        print("📈 Monitoring sync progress...")
        start_time = time.time()
        last_count = initial_count
        
        while time.time() - start_time < 120:  # 2 minutes
            current_count = get_email_count()
            
            if current_count != last_count:
                print(f"   📊 Progress: {current_count} emails (+{current_count - last_count})")
                last_count = current_count
                
                # If we've synced a significant number, we're making progress
                if current_count > initial_count + 100:
                    print("   🎉 Sync is working! Continuing to monitor...")
            
            time.sleep(5)
        
        # Send 'q' to quit TUImail
        print("🛑 Stopping TUImail...")
        process.stdin.write('q')
        process.stdin.flush()
        
        # Wait for process to finish
        process.wait(timeout=10)
        
    except Exception as e:
        print(f"⚠️  Error: {e}")
        process.terminate()
        process.wait()
    
    final_count = get_email_count()
    print(f"\n📊 Final Results:")
    print(f"   Initial: {initial_count} emails")
    print(f"   Final:   {final_count} emails")
    print(f"   Synced:  {final_count - initial_count} new emails")
    
    if final_count > initial_count:
        print("   ✅ SUCCESS: Sync is working!")
        progress = (final_count / 2249) * 100
        print(f"   📈 Progress: {progress:.1f}% of expected 2249 emails")
    else:
        print("   ❌ No progress made")
        print("   💡 Try running TUImail manually and pressing 'r' to refresh")

if __name__ == "__main__":
    main()
