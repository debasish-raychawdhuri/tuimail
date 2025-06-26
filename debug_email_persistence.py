#!/usr/bin/env python3

import sqlite3
import os
import json
from datetime import datetime

def check_database():
    """Check the email database for stored emails"""
    cache_dir = os.path.expanduser("~/.cache/tuimail")
    db_path = os.path.join(cache_dir, "emails.db")
    
    if not os.path.exists(db_path):
        print(f"❌ Database not found at {db_path}")
        return
    
    print(f"✅ Database found at {db_path}")
    
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        
        # Check if emails table exists
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='emails'")
        if not cursor.fetchone():
            print("❌ No emails table found")
            return
        
        # Count total emails
        cursor.execute("SELECT COUNT(*) FROM emails")
        total_emails = cursor.fetchone()[0]
        print(f"📧 Total emails in database: {total_emails}")
        
        if total_emails > 0:
            # Show recent emails by account/folder
            cursor.execute("""
                SELECT account_email, folder, COUNT(*) as count, 
                       MAX(date_received) as latest_timestamp
                FROM emails 
                GROUP BY account_email, folder 
                ORDER BY latest_timestamp DESC
            """)
            
            print("\n📁 Emails by account/folder:")
            for row in cursor.fetchall():
                account, folder, count, latest_ts = row
                latest_date = datetime.fromtimestamp(latest_ts) if latest_ts else "Unknown"
                print(f"  {account}/{folder}: {count} emails (latest: {latest_date})")
            
            # Show most recent 5 emails
            cursor.execute("""
                SELECT account_email, folder, subject, from_addresses, date_received
                FROM emails 
                ORDER BY date_received DESC 
                LIMIT 5
            """)
            
            print("\n📬 Most recent 5 emails:")
            for row in cursor.fetchall():
                account, folder, subject, from_addr, date_ts = row
                date_str = datetime.fromtimestamp(date_ts).strftime("%Y-%m-%d %H:%M:%S") if date_ts else "Unknown"
                subject_short = (subject[:50] + "...") if subject and len(subject) > 50 else (subject or "No subject")
                print(f"  {date_str} | {account}/{folder} | From: {from_addr} | {subject_short}")
        
        conn.close()
        
    except Exception as e:
        print(f"❌ Error checking database: {e}")

def check_config():
    """Check the configuration file"""
    config_path = os.path.expanduser("~/.config/tuimail/config.json")
    
    if not os.path.exists(config_path):
        print(f"❌ Config not found at {config_path}")
        return
    
    print(f"✅ Config found at {config_path}")
    
    try:
        with open(config_path, 'r') as f:
            config = json.load(f)
        
        accounts = config.get('accounts', [])
        print(f"👤 Configured accounts: {len(accounts)}")
        
        for i, account in enumerate(accounts):
            email = account.get('email', 'Unknown')
            name = account.get('name', 'Unknown')
            is_default = (i == config.get('default_account', 0))
            print(f"  {i}: {name} <{email}> {'(default)' if is_default else ''}")
            
    except Exception as e:
        print(f"❌ Error reading config: {e}")

def check_debug_log():
    """Check the debug log for recent activity"""
    log_path = "/tmp/tuimail_debug.log"
    
    if not os.path.exists(log_path):
        print(f"❌ Debug log not found at {log_path}")
        return
    
    print(f"✅ Debug log found at {log_path}")
    
    try:
        with open(log_path, 'r') as f:
            lines = f.readlines()
        
        # Show last 10 lines
        print("\n📝 Last 10 debug log entries:")
        for line in lines[-10:]:
            print(f"  {line.strip()}")
            
    except Exception as e:
        print(f"❌ Error reading debug log: {e}")

def main():
    print("🔍 TUImail Email Persistence Debug Tool")
    print("=" * 50)
    
    print("\n1. Checking configuration...")
    check_config()
    
    print("\n2. Checking database...")
    check_database()
    
    print("\n3. Checking debug log...")
    check_debug_log()
    
    print("\n" + "=" * 50)
    print("Debug complete!")
    print("\nIf emails are disappearing:")
    print("1. Check if database has emails but UI shows empty")
    print("2. Look for sync tracker issues in debug log")
    print("3. Check if refresh_emails_from_database is being called")

if __name__ == "__main__":
    main()
