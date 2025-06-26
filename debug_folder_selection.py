#!/usr/bin/env python3

import sqlite3
import os
import sys

def check_folder_consistency():
    """Check for folder consistency issues in the database"""
    
    # Find the database file
    db_path = os.path.expanduser("~/.cache/tuimail/emails.db")
    if not os.path.exists(db_path):
        print("❌ Database not found at:", db_path)
        # Try account-specific databases
        cache_dir = os.path.expanduser("~/.cache/tuimail/")
        if os.path.exists(cache_dir):
            for item in os.listdir(cache_dir):
                item_path = os.path.join(cache_dir, item)
                if os.path.isdir(item_path):
                    db_file = os.path.join(item_path, "emails.db")
                    if os.path.exists(db_file):
                        print(f"📁 Found account database: {db_file}")
                        db_path = db_file
                        break
        if not os.path.exists(db_path):
            return
    
    print("🔍 Checking folder consistency in TUImail database")
    print("=" * 50)
    
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        
        # Check pending operations
        print("\n📋 PENDING OPERATIONS:")
        cursor.execute("""
            SELECT id, account_email, operation_type, email_uid, folder, target_folder, created_at
            FROM email_operations 
            WHERE processed = FALSE 
            ORDER BY created_at DESC
        """)
        
        operations = cursor.fetchall()
        if operations:
            for op in operations:
                op_id, account, op_type, uid, folder, target, created = op
                print(f"  Operation {op_id}: {op_type} UID {uid} in folder '{folder}' (account: {account})")
        else:
            print("  ✅ No pending operations")
        
        # Check recent emails and their folders
        print("\n📧 RECENT EMAILS AND FOLDERS:")
        cursor.execute("""
            SELECT account_email, folder, uid, subject, seen
            FROM emails 
            ORDER BY date_received DESC 
            LIMIT 10
        """)
        
        emails = cursor.fetchall()
        if emails:
            for email in emails:
                account, folder, uid, subject, seen = email
                status = "READ" if seen else "UNREAD"
                print(f"  UID {uid}: {status} in '{folder}' - {subject[:50]}...")
        else:
            print("  ❌ No emails found")
        
        # Check for folder name variations
        print("\n📁 FOLDER VARIATIONS:")
        cursor.execute("""
            SELECT DISTINCT folder, COUNT(*) as count
            FROM emails 
            GROUP BY folder
            ORDER BY count DESC
        """)
        
        folders = cursor.fetchall()
        for folder, count in folders:
            print(f"  '{folder}': {count} emails")
        
        # Check for operations with mismatched folders
        print("\n🔍 CHECKING FOR FOLDER MISMATCHES:")
        cursor.execute("""
            SELECT DISTINCT eo.folder as op_folder, e.folder as email_folder, eo.email_uid
            FROM email_operations eo
            LEFT JOIN emails e ON eo.email_uid = e.uid AND eo.account_email = e.account_email
            WHERE eo.processed = FALSE AND eo.folder != e.folder
        """)
        
        mismatches = cursor.fetchall()
        if mismatches:
            print("  ❌ FOLDER MISMATCHES FOUND:")
            for op_folder, email_folder, uid in mismatches:
                print(f"    UID {uid}: Operation folder '{op_folder}' != Email folder '{email_folder}'")
        else:
            print("  ✅ No folder mismatches found")
        
        conn.close()
        
    except Exception as e:
        print(f"❌ Error checking database: {e}")

if __name__ == "__main__":
    check_folder_consistency()
