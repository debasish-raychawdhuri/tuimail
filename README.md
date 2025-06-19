# TUImail

A terminal-based email client built with Rust and Ratatui.

## Features

- **Terminal User Interface**: Clean, intuitive TUI for email management
- **Multiple Account Support**: Manage multiple email accounts
- **IMAP Support**: Connect to IMAP servers (Gmail, Outlook, etc.)
- **Email Composition**: Compose and send emails with attachments
- **Attachment Management**: Save and attach files with enhanced file browser
- **Spell Checking**: Built-in spell checker for email composition with visual highlighting
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

#### Spell Checking in Compose Mode
- `Alt+S`: Toggle spell checking on/off
- `Alt+G`: Show spelling suggestions for word at cursor
- `Alt+D`: Add word at cursor to personal dictionary

When spell suggestions are shown:
- `↑/↓`: Navigate suggestions
- `Enter`: Apply selected suggestion
- `Esc`: Cancel suggestions

## Spell Checking

TUImail includes a practical built-in spell checker with **~15,000 carefully curated English words** that helps you write error-free emails:

### Features
- **Practical Dictionary**: ~15,000 curated English words optimized for real-world email composition
- **Smart Word Selection**: Google's 10,000 most common English words + technical terms + everyday vocabulary
- **Real-time checking**: Spell checking as you type
- **Visual Highlighting**: Misspelled words are highlighted with a red background
- **Relevant suggestions**: Context-aware spelling suggestions for common misspellings
- **Personal dictionary**: Add custom words to avoid false positives
- **Configurable**: Enable/disable spell checking as needed
- **Performance optimized**: Efficient HashSet-based word lookup

### Dictionary Composition
The spell checker uses a carefully curated dictionary that includes:
- **Google's 10,000 most common English words**: Covers 99% of everyday vocabulary
- **Technical & business terms**: Email, software, technology, and professional terminology
- **Common everyday words**: Animals, colors, food, weather, time, places, etc.
- **Contractions**: don't, won't, can't, I'm, you're, etc.
- **Proper capitalization**: Both lowercase and capitalized versions of words

### Why This Approach?
Unlike comprehensive dictionaries with 400,000+ words that include obscure terms, archaic words, and proper nouns, our curated approach:
- ✅ **Catches real typos**: "Ther" → suggests "there"
- ✅ **Avoids false positives**: Doesn't include every obscure word that might be technically correct
- ✅ **Provides relevant suggestions**: Focuses on words people actually use in emails
- ✅ **Fast performance**: Smaller dictionary means faster lookups and suggestions
- ✅ **Practical accuracy**: Optimized for real-world email composition

### How It Works
- Spell checking is enabled by default for Subject and Body fields
- Misspelled words are detected as you type and highlighted with a red background
- The status bar shows spell check information and error count
- Use `Alt+G` to get suggestions for the word at your cursor
- Use `Alt+D` to add words to your personal dictionary

### Spell Check Status Bar
The bottom of the compose window shows:
- Current spell check status (enabled/disabled)
- Number of spelling errors found
- Spelling accuracy percentage
- Available keyboard shortcuts

### Smart Word Detection
The spell checker intelligently skips:
- Email addresses (user@domain.com)
- URLs (http://example.com)
- All-uppercase words (like acronyms)
- Words containing numbers
- Very short words (< 2 characters)

### Example Coverage
The dictionary correctly recognizes:
- **Common words**: the, there, was, have, with, from, they, etc.
- **Technical terms**: email, website, software, database, server, etc.
- **Business vocabulary**: meeting, project, deadline, invoice, etc.
- **Everyday words**: brown, crow, house, car, food, weather, etc.
- **Contractions**: don't, can't, I'm, you're, etc.

And correctly flags as misspellings:
- **Common typos**: "Ther" (should be "There"), "teh" (should be "the")
- **Misspellings**: "recieve" (should be "receive"), "seperate" (should be "separate")
- **Nonsense words**: "asdfgh", "qwerty", etc.

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
