# TUImail

A terminal-based email client built with Rust and Ratatui.

## Features

- **Terminal User Interface**: Clean, intuitive TUI for email management
- **Multiple Account Support**: Manage multiple email accounts
- **IMAP Support**: Connect to IMAP servers (Gmail, Outlook, etc.)
- **Email Composition**: Compose and send emails with attachments
- **Attachment Management**: Save and attach files with enhanced file browser
- **Secure Credentials**: Encrypted password storage using system keyring
- **Folder Navigation**: Browse email folders and organize messages

## Installation

### From Source

```bash
git clone https://github.com/debasish-raychawdhuri/tuimail.git
cd tuimail
cargo build --release
```

The binary will be available at `target/release/tuimail`.

## Usage

### First Run

```bash
tuimail add-account
```

Follow the prompts to add your email account.

### Running TUImail

```bash
tuimail
```

### Configuration

Configuration is stored in `~/.config/tuimail/config.json`.

### Debug Mode

For troubleshooting:

```bash
EMAIL_DEBUG=1 tuimail
```

Debug logs are written to `/tmp/tuimail_debug.log`.

## Key Bindings

### Main Interface
- `↑/↓`: Navigate emails
- `Enter`: View selected email
- `c`: Compose new email
- `r`: Refresh emails
- `f`: Browse folders
- `s`: Settings
- `?`: Help
- `q`: Quit

### Email View
- `Tab`: Navigate between email content and attachments
- `s`: Save selected attachment
- `Esc`: Return to email list

### File Browser (Save Mode)
- `↑/↓`: Navigate files/folders
- `Enter`: Select folder or edit filename
- `f`: Edit filename
- `s`: Save with current filename
- `q`: Quick save to Downloads
- `Esc`: Cancel

### Compose Mode
- `Ctrl+S`: Send email
- `Tab`: Navigate between fields
- `Esc`: Cancel composition

## Supported Email Providers

- Gmail (IMAP)
- Outlook/Hotmail (IMAP)
- Yahoo Mail (IMAP)
- Any IMAP-compatible email provider

## Requirements

- Rust 1.70+
- System keyring support (for secure password storage)

## License

This project is licensed under the MIT License.
