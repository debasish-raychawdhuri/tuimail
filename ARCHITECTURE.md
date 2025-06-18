# TUImail Architecture Documentation

## Overview

TUImail is a terminal-based email client built with Rust and Ratatui. It provides a clean TUI interface for managing multiple email accounts with IMAP/SMTP support, real-time notifications via IDLE, and secure credential storage.

## Module Structure

```
src/
├── main.rs              # Entry point, CLI parsing, terminal setup
├── app.rs               # Core application state and logic
├── ui.rs                # Terminal UI rendering with Ratatui
├── email.rs             # Email operations (IMAP/SMTP/IDLE)
├── config.rs            # Configuration management
├── credentials.rs       # Secure password storage
└── lib.rs               # Library exports
```

## Dependencies

### Core Dependencies
- **ratatui**: Terminal UI framework for rendering
- **crossterm**: Cross-platform terminal manipulation
- **clap**: Command-line argument parsing
- **tokio**: Async runtime (minimal usage)

### Email & Network
- **imap**: IMAP client for email fetching
- **lettre**: SMTP client for sending emails
- **native-tls**: TLS/SSL support for secure connections
- **mail-parser**: Email parsing and content extraction

### Storage & Security
- **keyring**: Secure credential storage using system keyring
- **serde**: Serialization for configuration
- **chrono**: Date/time handling

### Utilities
- **anyhow**: Error handling
- **thiserror**: Custom error types
- **shellexpand**: Path expansion (~/.config)

## Core Data Structures

### App State (`app.rs`)
```rust
pub struct App {
    // Configuration and accounts
    config: Config,
    accounts: HashMap<usize, AccountData>,
    current_account_idx: usize,
    
    // UI state
    mode: AppMode,
    focus: FocusPanel,
    emails: Vec<Email>,
    selected_email_idx: Option<usize>,
    
    // Background processing
    email_receiver: Option<Receiver<Vec<Email>>>,
    fetcher_running: Option<Arc<Mutex<bool>>>,
    
    // File operations
    file_browser_items: Vec<FileItem>,
    file_browser_selected: usize,
}
```

### Email Client (`email.rs`)
```rust
pub struct EmailClient {
    account: EmailAccount,
    cache_dir: String,
    credentials: SecureCredentials,
}
```

### Configuration (`config.rs`)
```rust
pub struct Config {
    accounts: Vec<EmailAccount>,
    default_account: Option<usize>,
}

pub struct EmailAccount {
    name: String,
    email: String,
    imap_server: String,
    smtp_server: String,
    // ... connection details
}
```

## Application Flow

### 1. Application Startup

```
main() 
├── Parse CLI arguments (clap)
├── Handle subcommands (add-account, etc.)
└── run_tui_mode()
    ├── Load configuration (~/.config/tuimail/config.json)
    ├── Initialize secure credentials (system keyring)
    ├── Setup terminal (crossterm + ratatui)
    ├── Create App instance
    ├── Initialize first account
    └── Start main event loop
```

**Call Flow:**
1. `main.rs::main()` → Parse CLI with clap
2. `main.rs::run_tui_mode()` → Setup terminal and app
3. `app.rs::App::new()` → Create app state
4. `app.rs::init()` → Load config and initialize accounts
5. `main.rs::run_app()` → Start main event loop

### 2. Account Initialization

```
App::init()
├── Load config from ~/.config/tuimail/config.json
├── Initialize credentials manager
├── For each account:
│   ├── Create EmailClient
│   ├── Test IMAP connection
│   ├── Load folder list
│   └── Cache initial emails
└── Start background IDLE fetching
```

**Call Flow:**
1. `app.rs::init()` → Entry point
2. `config.rs::Config::load()` → Load configuration
3. `credentials.rs::SecureCredentials::new()` → Setup keyring
4. `app.rs::init_account()` → Per-account setup
5. `email.rs::EmailClient::new()` → Create email client
6. `email.rs::list_folders()` → Get IMAP folders
7. `email.rs::fetch_emails()` → Get initial emails
8. `app.rs::start_background_email_fetching()` → Start IDLE

### 3. Main Event Loop

```
run_app() loop:
├── Check for new emails (IDLE notifications)
├── Draw UI (ratatui)
├── Poll for input events (100ms timeout)
├── Handle key events
├── Update app state
└── Check exit condition
```

**Call Flow:**
1. `main.rs::run_app()` → Main loop
2. `app.rs::check_for_new_emails()` → Check IDLE channel
3. `ui.rs::ui()` → Render terminal interface
4. `crossterm::event::poll()` → Check for input
5. `app.rs::handle_key_event()` → Process user input
6. `app.rs::tick()` → Update app state

### 4. Email Operations

#### Fetching Emails
```
User presses 'r' (refresh)
├── app.rs::handle_key_event() → KeyCode::Char('r')
├── app.rs::load_emails_for_selected_folder()
├── email.rs::fetch_emails()
│   ├── Connect to IMAP server
│   ├── Select folder
│   ├── Search for emails
│   ├── Fetch email metadata
│   └── Parse email content
└── Update UI with new emails
```

#### Sending Emails
```
User presses 'c' (compose)
├── app.rs::handle_key_event() → KeyCode::Char('c')
├── Switch to compose mode
├── User fills in recipient, subject, body
├── User presses Ctrl+S
├── app.rs::send_email()
├── email.rs::send_email()
│   ├── Create SMTP connection
│   ├── Build email message (lettre)
│   ├── Add attachments if any
│   └── Send via SMTP
└── Return to normal mode
```

#### Mark as Read
```
User views email
├── app.rs::handle_key_event() → Enter key
├── Switch to email view mode
├── app.rs::mark_current_email_as_read()
├── email.rs::mark_as_read()
│   ├── Connect to IMAP server
│   ├── Select email folder
│   ├── Set \Seen flag (with retry logic)
│   └── Handle connection errors
└── Update email status in UI
```

### 5. Background IDLE Processing

```
Background Thread (per account):
├── email.rs::run_idle_session()
├── Connect to IMAP server
├── Select folder (usually INBOX)
├── Start IDLE command
├── Wait for server notifications
├── On notification:
│   ├── Fetch new emails
│   ├── Send via channel to main thread
│   └── Continue IDLE loop
└── Handle disconnections and errors
```

**Integration with Main Thread:**
1. `app.rs::start_background_email_fetching()` → Spawn thread
2. `email.rs::run_idle_session()` → Background IDLE loop
3. `std::sync::mpsc::channel` → Communication channel
4. `app.rs::check_for_new_emails()` → Main thread polling
5. Update `self.emails` and refresh UI

### 6. File Operations

#### Saving Attachments
```
User presses 's' on attachment
├── app.rs::handle_key_event() → KeyCode::Char('s')
├── Switch to file browser mode
├── app.rs::init_file_browser()
├── User navigates directories
├── User presses 'f' to edit filename
├── User enters filename and presses Enter
├── app.rs::save_attachment()
├── Write attachment data to file
└── Return to email view
```

#### File Browser Navigation
```
File Browser Mode:
├── ui.rs::render_file_browser() → Display files/folders
├── User presses Up/Down → Navigate items
├── User presses Enter → Enter folder or select file
├── User presses 'f' → Edit filename mode
├── User types filename → Update filename buffer
├── User presses 's' → Save with current filename
└── User presses Esc → Cancel and return
```

### 7. Account Management

#### Switching Accounts
```
User presses 'n' (next account)
├── app.rs::handle_key_event() → KeyCode::Char('n')
├── app.rs::rotate_to_next_account()
├── Calculate next account index
├── Check if account already initialized
├── Load cached emails or fetch from server
├── Start IDLE for new account
├── Update UI to show new account
└── Reset email selection
```

#### Adding New Account
```
Command: tuimail add-account
├── main.rs::main() → Subcommand::AddAccount
├── Prompt for account details (interactive)
├── Test IMAP/SMTP connections
├── Store credentials securely (keyring)
├── Save account to config file
└── Exit with success message
```

## Error Handling Strategy

### Layered Error Handling
1. **Network Errors**: Retry logic with exponential backoff
2. **IMAP Errors**: Connection pooling and reconnection
3. **UI Errors**: Graceful degradation with error messages
4. **File Errors**: User-friendly error reporting

### Error Types
```rust
pub enum AppError {
    IoError(std::io::Error),
    EmailError(EmailError),
    ConfigError(String),
    CredentialError(String),
}

pub enum EmailError {
    ImapError(String),
    SmtpError(String),
    ParsingError(String),
    ConnectionError(String),
}
```

## Performance Optimizations

### Caching Strategy
- **Email Caching**: Store emails per account/folder
- **Connection Reuse**: Maintain IMAP connections when possible
- **Lazy Loading**: Load email content on demand
- **Background Fetching**: IDLE for real-time updates

### Memory Management
- **Bounded Collections**: Limit email list size
- **Attachment Streaming**: Stream large attachments
- **Connection Pooling**: Reuse IMAP/SMTP connections
- **Cleanup**: Proper resource disposal on exit

## Security Considerations

### Credential Storage
- **System Keyring**: Use OS-native secure storage
- **Fallback Storage**: Encrypted file if keyring unavailable
- **No Plaintext**: Never store passwords in plain text
- **Memory Safety**: Clear sensitive data from memory

### Network Security
- **TLS/SSL**: Enforce encrypted connections
- **Certificate Validation**: Verify server certificates
- **Connection Timeouts**: Prevent hanging connections
- **Error Sanitization**: Don't leak sensitive info in errors

## Configuration Management

### File Locations
- **Config**: `~/.config/tuimail/config.json`
- **Cache**: `~/.cache/tuimail/`
- **Debug Log**: `/tmp/tuimail_debug.log`

### Configuration Structure
```json
{
  "accounts": [
    {
      "name": "Work Email",
      "email": "user@company.com",
      "imap_server": "imap.company.com",
      "imap_port": 993,
      "imap_security": "SSL",
      "smtp_server": "smtp.company.com",
      "smtp_port": 587,
      "smtp_security": "StartTLS"
    }
  ],
  "default_account": 0
}
```

## Debugging and Logging

### Debug Mode
```bash
EMAIL_DEBUG=1 tuimail
```

### Log Locations
- **Debug Log**: `/tmp/tuimail_debug.log`
- **Error Messages**: Displayed in UI status bar
- **Network Traces**: IMAP/SMTP command logging

### Debug Information
- Connection establishment/teardown
- IMAP command sequences
- IDLE session lifecycle
- Email parsing details
- File operations
- Error stack traces

This architecture provides a solid foundation for a reliable, secure, and performant terminal email client with real-time capabilities.
