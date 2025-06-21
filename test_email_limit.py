#!/usr/bin/env python3

import sqlite3
import os
import time

# Test the email loading performance by checking database queries
def test_email_loading():
    print("🔍 Testing Email Loading Performance")
    print("=" * 40)
    
    # Path to the email database
    db_path = os.path.expanduser("~/.cache/tuimail/214054001_at_iitb_ac_in/emails.db")
    
    if not os.path.exists(db_path):
        print("❌ Database not found at:", db_path)
        return
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Test 1: Count total emails
    print("📊 Total emails in INBOX:")
    start_time = time.time()
    cursor.execute("SELECT COUNT(*) FROM emails WHERE account_email = '214054001@iitb.ac.in' AND folder = 'INBOX'")
    total_count = cursor.fetchone()[0]
    count_time = time.time() - start_time
    print(f"   Total: {total_count} emails (took {count_time:.3f}s)")
    
    # Test 2: Load ALL emails (old approach)
    print("\n🐌 Loading ALL emails (old approach):")
    start_time = time.time()
    cursor.execute("""
        SELECT uid, message_id, subject, from_addresses, to_addresses, 
               cc_addresses, bcc_addresses, date_received, body_text, body_html,
               flags, headers, seen
        FROM emails 
        WHERE account_email = '214054001@iitb.ac.in' AND folder = 'INBOX' 
        ORDER BY date_received DESC
    """)
    all_emails = cursor.fetchall()
    all_time = time.time() - start_time
    print(f"   Loaded: {len(all_emails)} emails (took {all_time:.3f}s)")
    
    # Test 3: Load limited emails (new approach)
    print("\n🚀 Loading LIMITED emails (new approach - 1000 limit):")
    start_time = time.time()
    cursor.execute("""
        SELECT uid, message_id, subject, from_addresses, to_addresses, 
               cc_addresses, bcc_addresses, date_received, body_text, body_html,
               flags, headers, seen
        FROM emails 
        WHERE account_email = '214054001@iitb.ac.in' AND folder = 'INBOX' 
        ORDER BY date_received DESC
        LIMIT 1000
    """)
    limited_emails = cursor.fetchall()
    limited_time = time.time() - start_time
    print(f"   Loaded: {len(limited_emails)} emails (took {limited_time:.3f}s)")
    
    # Test 4: Load paginated emails (50 per page)
    print("\n📄 Loading PAGINATED emails (50 per page):")
    start_time = time.time()
    cursor.execute("""
        SELECT uid, message_id, subject, from_addresses, to_addresses, 
               cc_addresses, bcc_addresses, date_received, body_text, body_html,
               flags, headers, seen
        FROM emails 
        WHERE account_email = '214054001@iitb.ac.in' AND folder = 'INBOX' 
        ORDER BY date_received DESC
        LIMIT 50 OFFSET 0
    """)
    page_emails = cursor.fetchall()
    page_time = time.time() - start_time
    print(f"   Loaded: {len(page_emails)} emails (took {page_time:.3f}s)")
    
    conn.close()
    
    # Performance comparison
    print("\n📈 Performance Comparison:")
    print(f"   ALL emails:    {all_time:.3f}s ({len(all_emails)} emails)")
    print(f"   LIMITED:       {limited_time:.3f}s ({len(limited_emails)} emails)")
    print(f"   PAGINATED:     {page_time:.3f}s ({len(page_emails)} emails)")
    
    if all_time > 0:
        improvement_limited = ((all_time - limited_time) / all_time) * 100
        improvement_paginated = ((all_time - page_time) / all_time) * 100
        print(f"\n🎯 Performance Improvements:")
        print(f"   Limited approach:   {improvement_limited:.1f}% faster")
        print(f"   Paginated approach: {improvement_paginated:.1f}% faster")
    
    print(f"\n✅ The app should now load only {len(limited_emails)} emails instead of {total_count}")
    print("✅ This should make the UI much more responsive!")

if __name__ == "__main__":
    test_email_loading()
