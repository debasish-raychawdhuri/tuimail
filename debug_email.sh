#!/bin/bash

# Clear any existing debug log
rm -f /tmp/email_client_debug.log

echo "Starting email client with debug logging..."
echo "Debug log will be written to: /tmp/email_client_debug.log"
echo ""
echo "To view the log in real-time in another terminal, run:"
echo "  tail -f /tmp/email_client_debug.log"
echo ""
echo "Press Ctrl+C to stop the email client and view the debug log."
echo ""

# Run the email client with debug enabled
EMAIL_DEBUG=1 ./email_client

echo ""
echo "=== DEBUG LOG CONTENTS ==="
if [ -f /tmp/email_client_debug.log ]; then
    cat /tmp/email_client_debug.log
else
    echo "No debug log found."
fi
