#!/bin/bash

echo "Testing email persistence fix..."
echo "This will run the email client with debug logging enabled."
echo "Check if emails appear and stay visible (don't disappear after 2 seconds)."
echo ""
echo "Press Ctrl+C to stop the test."
echo ""

# Enable debug logging
export EMAIL_DEBUG=1

# Run the email client
cd /home/debasish/rust/email_client
./target/release/tuimail

echo ""
echo "Test completed. Check /tmp/tuimail_debug.log for debug information."
