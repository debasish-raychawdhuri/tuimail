#!/usr/bin/env python3

import subprocess
import time
import os
import signal

def test_email_persistence():
    """Test if emails persist in the UI"""
    print("🧪 Testing email persistence...")
    
    # Clear debug log
    debug_log = "/tmp/tuimail_debug.log"
    if os.path.exists(debug_log):
        os.remove(debug_log)
    
    # Start the email client with debug logging
    env = os.environ.copy()
    env['EMAIL_DEBUG'] = '1'
    
    print("Starting email client...")
    process = subprocess.Popen(
        ['./target/release/tuimail'],
        cwd='/home/debasish/rust/email_client',
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    # Let it run for 15 seconds
    print("Letting it run for 15 seconds...")
    time.sleep(15)
    
    # Terminate the process
    print("Terminating process...")
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    
    # Check debug log
    print("\n📝 Checking debug log...")
    if os.path.exists(debug_log):
        with open(debug_log, 'r') as f:
            lines = f.readlines()
        
        print(f"Found {len(lines)} debug log entries:")
        for line in lines[-20:]:  # Show last 20 lines
            print(f"  {line.strip()}")
    else:
        print("❌ No debug log found")
    
    print("\n✅ Test completed")

if __name__ == "__main__":
    test_email_persistence()
