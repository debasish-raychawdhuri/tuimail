#!/bin/bash

# TUImail Sync Daemon Runner Script

set -e

echo "Building TUImail with sync daemon..."
cargo build --release

echo "TUImail Sync Daemon - Email Synchronization Service"
echo "=================================================="
echo

# Check if configuration exists
CONFIG_FILE="$HOME/.config/tuimail/config.json"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "❌ No configuration found at $CONFIG_FILE"
    echo "Please run 'tuimail add-account' first to set up your email accounts."
    exit 1
fi

echo "✅ Configuration found"

# Show available options
echo
echo "Available options:"
echo "  1. Run sync once and exit"
echo "  2. Run sync daemon (foreground)"
echo "  3. Run sync daemon (background)"
echo "  4. Show sync status"
echo "  5. Stop background daemon"
echo

read -p "Choose an option (1-5): " choice

case $choice in
    1)
        echo "Running one-time sync..."
        ./target/release/tuimail-sync --once
        ;;
    2)
        echo "Starting sync daemon in foreground..."
        echo "Press Ctrl+C to stop"
        ./target/release/tuimail-sync
        ;;
    3)
        echo "Starting sync daemon in background..."
        ./target/release/tuimail-sync --daemon &
        DAEMON_PID=$!
        echo "Sync daemon started with PID: $DAEMON_PID"
        echo "To stop: kill $DAEMON_PID"
        echo $DAEMON_PID > /tmp/tuimail-sync.pid
        ;;
    4)
        echo "Checking sync status..."
        if [ -f /tmp/tuimail-sync.pid ]; then
            PID=$(cat /tmp/tuimail-sync.pid)
            if ps -p $PID > /dev/null 2>&1; then
                echo "✅ Sync daemon is running (PID: $PID)"
            else
                echo "❌ Sync daemon is not running"
                rm -f /tmp/tuimail-sync.pid
            fi
        else
            echo "❌ No sync daemon PID file found"
        fi
        
        # Show database stats
        DB_FILE="$HOME/.cache/tuimail/emails.db"
        if [ -f "$DB_FILE" ]; then
            echo
            echo "Database statistics:"
            sqlite3 "$DB_FILE" "SELECT 'Total emails: ' || COUNT(*) FROM emails;"
            sqlite3 "$DB_FILE" "SELECT 'Accounts: ' || COUNT(DISTINCT account_email) FROM emails;"
            sqlite3 "$DB_FILE" "SELECT 'Database size: ' || ROUND(page_count * page_size / 1024.0 / 1024.0, 2) || ' MB' FROM pragma_page_count(), pragma_page_size();"
        fi
        ;;
    5)
        echo "Stopping background daemon..."
        if [ -f /tmp/tuimail-sync.pid ]; then
            PID=$(cat /tmp/tuimail-sync.pid)
            if ps -p $PID > /dev/null 2>&1; then
                kill $PID
                echo "✅ Sync daemon stopped"
                rm -f /tmp/tuimail-sync.pid
            else
                echo "❌ Sync daemon was not running"
                rm -f /tmp/tuimail-sync.pid
            fi
        else
            echo "❌ No sync daemon PID file found"
        fi
        ;;
    *)
        echo "Invalid option"
        exit 1
        ;;
esac

echo
echo "Done!"
