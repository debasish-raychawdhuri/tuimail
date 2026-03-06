# TUImail Architecture

A terminal-based email client built in Rust using Ratatui, IMAP, and SMTP.
Total: ~13,500 lines across 12 source files.

## Project Structure

```
src/
├── main.rs          (590 lines)  Entry point, CLI, event loop
├── app.rs          (4748 lines)  Core state machine, key handling, sync
├── email.rs        (3084 lines)  IMAP/SMTP client, email parsing
├── database.rs     (1636 lines)  SQLite cache, operation queue
├── ui.rs           (1540 lines)  Ratatui rendering
├── spellcheck.rs    (846 lines)  Dictionary-based spell checker
├── config.rs        (290 lines)  JSON config, account settings
├── credentials.rs   (299 lines)  Keyring + encrypted fallback
├── async_grammar.rs (199 lines)  Async grammar check wrapper
├── grammarcheck.rs  (170 lines)  Grammar checker (nlprule placeholder)
├── lib.rs            (15 lines)  Module exports
└── test_parsing.rs   (63 lines)  Debug utility
```

## Dependencies

### Core
- **ratatui** + **crossterm**: Terminal UI framework
- **clap**: CLI argument parsing
- **tokio**: Async runtime (for grammar checking)

### Email & Network
- **imap**: IMAP client for email fetching and IDLE
- **lettre**: SMTP client for sending emails
- **native-tls**: TLS/SSL support
- **mail-parser**: RFC 5322 email parsing and MIME extraction

### Storage & Security
- **rusqlite**: SQLite database (bundled)
- **keyring**: System keyring integration (GNOME, KDE, macOS)
- **serde** / **serde_json**: Serialization
- **chrono**: Date/time handling

### Text Processing
- **nlprule**: Grammar checking (via nlprule-build)

### Utilities
- **anyhow** / **thiserror**: Error handling
- **shellexpand**: Path expansion
- **dirs**: Platform-specific directories

---

## Module Overview

### main.rs -- Entry Point

- **CLI**: `clap`-based subcommands (`add-account`, `list-accounts`, `test-account`, `set-default-account`)
- **Setup**: Loads config from `~/.config/tuimail/config.json`, creates SQLite DB at `~/.cache/tuimail/emails.db`, enters raw terminal mode via `crossterm`
- **Event Loop** (`run_app`):
  - Polls keyboard/resize events with 1s timeout
  - Calls `app.refresh_emails_from_database()` every 30s
  - Debounces spell check (500ms), grammar check (2s), address parsing (300ms) in compose mode
  - Calls `ui::ui()` on every iteration to redraw
- **Shutdown**: Restores terminal, disables raw mode

### app.rs -- Application State Machine

#### Core Types

```
AppMode:      Normal | Compose | ViewEmail | FolderList | AccountSettings
              | Help | DeleteConfirm | Search

FocusPanel:   FolderList | EmailList | ComposeForm

ComposeField: To | Cc | Bcc | Subject | Body
```

#### App Struct (key fields)

| Field | Type | Purpose |
|---|---|---|
| `config` | `Config` | User configuration |
| `credentials` | `SecureCredentials` | Password storage |
| `database` | `Arc<EmailDatabase>` | Main SQLite connection |
| `accounts` | `HashMap<usize, AccountData>` | Per-account state (folders, email cache, client) |
| `emails` | `Vec<EmailSummary>` | Current folder's email list (lightweight) |
| `viewed_email` | `Option<Email>` | Full email loaded on-demand |
| `compose_email` | `Email` | Draft being composed |
| `spell_checker` | `Option<SpellChecker>` | Spell check instance |
| `async_grammar_checker` | `Option<AsyncGrammarChecker>` | Grammar check instance |
| `folder_items` | `Vec<FolderItem>` | Hierarchical folder tree (expandable accounts) |
| `search_results` | `Vec<EmailSummary>` | Search hits |
| `ui_timestamps` | `HashMap<String, DateTime<Utc>>` | Prevents redundant sync |
| `sync_thread_running` | `Arc<AtomicBool>` | Background sync control |

#### Key Methods

- **`init()`** -- Initialize all accounts, load default INBOX
- **`handle_key_event()`** -- Dispatches to mode-specific handlers (Normal, Compose, ViewEmail, Search, etc.)
- **`load_emails_for_account_folder()`** -- Smart sync via IMAP, falls back to cached DB
- **`load_full_email(summary)`** -- Loads full `Email` (body + attachments) from per-account DB on demand
- **`refresh_emails_from_database()`** -- Periodic poll using per-account DB with `get_recent_email_summaries()`
- **`check_for_new_emails()`** -- Poll per-account DB for changes
- **`send_email()`** -- Compose and send via SMTP
- **`start_background_email_fetching()`** -- Spawns per-account sync thread with IDLE monitoring
- **`reply_to_email()` / `reply_all_to_email()` / `forward_email()`** -- Load `viewed_email`, construct reply/forward `Email`, enter Compose mode
- **`delete_selected_email()`** -- IMAP UID STORE +FLAGS (\Deleted) + EXPUNGE
- **`check_spelling()` / `request_grammar_check()`** -- Debounced text analysis
- **`tick()`** -- Clear expired info/error messages after 3s

### email.rs -- IMAP/SMTP Client

#### Data Structures

```rust
Email             // Full: id, subject, from/to/cc/bcc, date, body_text, body_html,
                  // attachments (Vec<u8>), flags, headers, seen, folder

EmailSummary      // Lightweight for list display: id, subject, from, date, seen,
                  // folder, has_attachments (bool). Never saved to database.

EmailAddress      // { name: Option<String>, address: String }
EmailAttachment   // { filename, content_type, data: Vec<u8> }
```

#### EmailClient

- **Connections**: `connect_imap_secure()` (TLS/993), `connect_imap_plain()`, `connect_imap_with_security()`
- **Folder ops**: `list_folders()`
- **Fetch strategies**:
  - `smart_sync(folder)` -- Determines strategy based on DB state (initial vs incremental vs recent)
  - `fetch_emails_incrementally_secure/plain()` -- First sync: all emails in batches of 500. Incremental: UIDs > last_uid
  - `fetch_new_emails_since_count()` -- Used by IDLE monitoring for live updates
- **Parsing**: `Email::from_parsed_email()` uses `mail_parser` for RFC 5322; `extract_attachments()` walks MIME parts checking Content-Disposition and Content-Type
- **Sending**: `send_smtp()` via `lettre` with multipart (text + attachments)
- **IDLE**: `start_idle_monitoring()` -- Persistent IMAP connection, wakes on server notifications, fetches new emails, saves to per-account DB
- **Per-account DB path**: `~/.cache/tuimail/{email_sanitized}/emails.db`

### database.rs -- SQLite Cache

#### Schema

```sql
emails (
    uid INTEGER, account_email TEXT, folder TEXT,
    message_id TEXT, subject TEXT,
    from_addresses JSON, to_addresses JSON, cc_addresses JSON, bcc_addresses JSON,
    date_received INTEGER, body_text TEXT, body_html TEXT,
    flags JSON, headers JSON, seen BOOLEAN,
    PRIMARY KEY(account_email, folder, uid)
)

attachments (
    id INTEGER PRIMARY KEY,
    account_email TEXT, folder TEXT, email_uid INTEGER,
    filename TEXT, content_type TEXT, data BLOB, size INTEGER,
    FOREIGN KEY → emails ON DELETE CASCADE
)

folder_metadata (
    account_email TEXT, folder TEXT,
    last_uid INTEGER, total_messages INTEGER, last_sync INTEGER,
    UNIQUE(account_email, folder)
)

email_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_email TEXT, operation_type TEXT,
    email_uid INTEGER, folder TEXT, target_folder TEXT,
    created_at INTEGER, processed BOOLEAN, error TEXT
)
```

#### Key Methods

| Method | Returns | Purpose |
|---|---|---|
| `save_emails()` | -- | Bulk INSERT OR REPLACE in transaction (deletes + re-inserts attachments) |
| `save_email()` | -- | Single email INSERT OR REPLACE |
| `load_emails()` | `Vec<Email>` | Full emails with attachments (used by sync paths) |
| `get_recent_email_summaries()` | `Vec<EmailSummary>` | Lightweight list with `EXISTS` subquery for `has_attachments` |
| `get_email_full()` | `Email` | Single full email with attachments (on-demand viewing) |
| `queue_email_operation()` | -- | Queue mark_read/delete/move for background processing |
| `get_pending_operations()` | `Vec<(...)>` | Retrieve unprocessed operations |
| `save_folder_metadata()` | -- | Track sync state (last_uid, total_messages) |

#### Indexes

- `idx_emails_account_folder` -- Folder listing
- `idx_emails_uid` -- UID lookup
- `idx_emails_date` -- Recent emails sorted by date
- `idx_attachments_email` -- Load attachments by email
- `idx_email_operations_processed` -- Find pending operations

### ui.rs -- Terminal Rendering

#### Screen Layout

```
+-- Title Bar (mode tabs) ----------------------------------+
+-- Main Content -------------------------------------------+
| +- Folder Tree -+ +- Email List / View / Compose ------+ |
| | > Account 1   | |                                    | |
| |   INBOX       | | (mode-dependent content)           | |
| |   Sent        | |                                    | |
| | > Account 2   | |                                    | |
| +---------------+ +------------------------------------+ |
+-- Status Bar ---------------------------------------------+
```

#### Render Dispatch

```
ui()
 +-- render_title_bar()
 +-- render_main_content()
 |    +-- Normal:    render_normal_mode()
 |    |               +-- render_folder_list()
 |    |               +-- render_email_list()    <-- reads app.emails (Vec<EmailSummary>)
 |    +-- ViewEmail: render_view_email_mode()    <-- reads app.viewed_email (full Email)
 |    |               +-- render_email_header()
 |    |               +-- render_email_attachments()
 |    |               +-- render_scrollable_email_body()
 |    +-- Compose:   render_compose_mode()
 |    |               +-- field editors + spell/grammar popups
 |    +-- Search:    render_search_mode()
 |    +-- Help:      render_help_mode()
 |    +-- Settings:  render_account_settings()
 +-- render_status_bar()
```

- **Selection style**: White background, black text, bold
- **Unread emails**: Green text
- **Attachment indicator**: has_attachments bool on EmailSummary

### config.rs -- Configuration

```rust
Config {
    accounts: Vec<EmailAccount>,  // IMAP/SMTP server, port, security, username
    default_account: usize,
    ui: UIConfig,                 // theme, show_headers, refresh_interval
}
```

- Security enums: `ImapSecurity { None, StartTLS, SSL }`, `SmtpSecurity { None, StartTLS, SSL }`
- Passwords NOT in config -- stored via `SecureCredentials`
- Location: `~/.config/tuimail/config.json`

### credentials.rs -- Secure Password Storage

- **Primary**: System keyring via `keyring` crate (GNOME Keyring, KDE Wallet, macOS Keychain)
- **Fallback**: XOR-encrypted files in `~/.config/tuimail/credentials/*.enc` with 0o600 permissions
- `store_password(account_id, type, password)` / `get_password(account_id, type)`

### spellcheck.rs -- Spell Checking

- Embedded dictionaries: google-10000-english + technical terms + common words (~15k total)
- O(1) word lookup via `HashSet<String>`
- Suggestions computed on-demand only (not during check pass)
- Algorithms: common misspelling patterns, 1-char insertion/deletion, substring transposition
- Limited to 5 suggestions, 500 dictionary iterations max

### grammarcheck.rs + async_grammar.rs -- Grammar Checking

- `GrammarChecker`: Placeholder wrapping `nlprule` (currently returns empty results)
- `AsyncGrammarChecker`: Tokio-based async wrapper with request/response channels
- Debounced: only processes latest request after 2s inactivity, cancels stale requests

---

## Call Chains

### Startup

```
main()
  -> parse CLI args (clap)
  -> Config::load("~/.config/tuimail/config.json")
  -> SecureCredentials::new()
  -> EmailDatabase::new("~/.cache/tuimail/emails.db")
  -> App::new(config, database)
  -> App::init()
      -> for each account:
          init_account(idx)
            -> EmailClient::new(account, credentials)
            -> client.list_folders()  [IMAP LIST]
            -> build folder_items tree
          load_emails_for_account_folder(idx, "INBOX")
            -> client.smart_sync("INBOX")
                -> determine_sync_strategy()
                   -> InitialSync | IncrementalSync | RecentSync
                -> fetch_emails_incrementally_secure/plain()
                   -> IMAP FETCH (RFC822 FLAGS UID) in batches of 500
                   -> parse_messages() -> Email::from_parsed_email()
                -> database.save_emails()  [per-account DB]
            -> database.get_recent_email_summaries()
               -> app.emails = summaries
      -> start_background_email_fetching()
          -> spawn thread per account
              -> client.start_idle_monitoring()
  -> run_app(terminal, app)
      -> event loop (poll 1s)
```

### Opening an Email

```
handle_key_event(Enter) in Normal mode
  -> summary = self.emails[selected_idx].clone()
  -> self.viewed_email = self.load_full_email(&summary)
      -> uid = summary.id.parse()
      -> try per-account DB: acct_db.get_email_full(account_email, folder, uid)
         -> SELECT from emails + attachments joined
      -> fallback: self.database.get_email_full(...)
  -> self.mode = ViewEmail
  -> mark as read:
      -> queue_email_operation("mark_read")  [deferred to background thread]
      -> update_seen_in_all_databases()

ui::render_view_email_mode()
  -> reads app.viewed_email (full Email with body + attachments)
  -> render_email_header(email)     -- From, To, Subject, Date
  -> render_email_attachments(email) -- if any, with save option
  -> render_scrollable_email_body(email) -- scrollable with up/down/PgUp/PgDn
```

### Composing and Sending

```
handle_key_event(Ctrl+N)
  -> mode = Compose, compose_email = Email::new()

[User types in To/Subject/Body fields]
  -> handle_compose_input(char)
  -> mark_keystroke() for debouncing

[After 500ms idle in main loop]
  -> update_spell_check()
      -> spell_checker.check_text(body_text)
      -> spell_errors = [SpellError{word, position, suggestions}]

[User presses Ctrl+Enter to send]
  -> send_email()
      -> parse address fields (to_text -> Vec<EmailAddress>)
      -> EmailClient::send_smtp(compose_email)
          -> lettre::Message::builder()
              -> .from() .to() .subject() .body()
              -> attach files as multipart
          -> lettre::SmtpTransport::relay()
          -> transport.send(message)
      -> mode = Normal, show_info("Email sent")
```

### Reply / Reply-All / Forward

```
handle_key_event('r' / 'R' / 'f') in ViewEmail mode
  -> reply_to_email() / reply_all_to_email() / forward_email()
      -> ensure self.viewed_email is loaded (load_full_email if needed)
      -> original = &self.viewed_email
      -> reply = Email::new()
      -> set subject (Re: / Fwd: prefix)
      -> set recipients:
          reply:     original sender (reply-to if present)
          reply-all: + all TO/CC recipients except self
          forward:   empty (user fills in)
      -> set In-Reply-To / References headers (for threading)
      -> quote original body with "> " prefix (reply) or forward header block
      -> forward: copy attachments from original
      -> self.compose_email = reply
      -> mode = Compose
```

### Background Email Sync

```
start_background_email_fetching()
  -> spawn thread:
      -> for each account:
          EmailClient::start_idle_monitoring(folder, database)
              -> IMAP login + SELECT folder
              -> loop:
                  -> session.idle().wait_with_timeout(30s)
                  -> on notification:
                      -> fetch_new_emails_since_count(folder, last_count)
                          -> IMAP FETCH (RFC822 FLAGS UID) for new messages
                          -> parse_messages()
                      -> database.save_emails()  [per-account DB]
                  -> on timeout:
                      -> NOOP to check connection health
                      -> reconnect if needed

[Every 30s in main event loop]
  -> app.refresh_emails_from_database()
      -> check has_new_emails_since_global() via sync_tracker
      -> if new emails detected:
          -> per-account DB: get_recent_email_summaries(500)
          -> update app.emails
          -> update ui_timestamps
```

### Delete Email

```
handle_key_event('d') in Normal mode
  -> mode = DeleteConfirm (show confirmation dialog)

handle_key_event('y') in DeleteConfirm mode
  -> delete_selected_email()
      -> summary = self.emails[idx]
      -> client.delete_email(summary.id, summary.folder)
          -> IMAP UID STORE +FLAGS (\Deleted)
          -> IMAP EXPUNGE
      -> self.emails.remove(idx)
      -> adjust selection
```

### Search

```
handle_key_event('/') in Normal mode
  -> mode = Search, save pre_search_emails

[User types query, presses Enter]
  -> perform_search(query)
      -> search in-memory: filter summaries by subject/from matching query
      -> search_results = matching summaries
      -> self.emails = search_results

handle_key_event(Esc) in Search mode
  -> restore self.emails from pre_search_emails
  -> mode = Normal
```

### Periodic Database Refresh

```
refresh_emails_from_database()  [called every 30s from main loop]
  -> get account_email, folder from current selection
  -> if UI email list is empty:
      -> per-account DB: get_recent_email_summaries(500)
      -> app.emails = summaries
  -> else:
      -> check has_new_emails_since_global() via sync_tracker
      -> if new emails detected:
          -> per-account DB: get_recent_email_summaries(500)
          -> app.emails = summaries
      -> update ui_timestamps
```

---

## Database Architecture

Two levels of SQLite databases:

```
~/.cache/tuimail/
├── emails.db                              Main DB (operation queue)
├── user_at_gmail_com/
│   └── emails.db                          Per-account DB (email data)
└── work_at_company_com/
    └── emails.db                          Per-account DB (email data)
```

1. **Main DB** (`~/.cache/tuimail/emails.db`)
   - Operation queue for deferred IMAP operations (mark_read, delete, move)
   - Fallback for cross-account queries

2. **Per-Account DB** (`~/.cache/tuimail/{email_sanitized}/emails.db`)
   - Primary email storage per account
   - Written by: IMAP sync (background thread + smart_sync)
   - Read by: UI for summaries (`get_recent_email_summaries`) and full email loading (`get_email_full`)
   - Same schema as main DB

The per-account DB is the source of truth for email data.

---

## EmailSummary vs Email

A key architectural pattern for memory efficiency:

```
EmailSummary (list display)          Email (full content)
  id                                   id
  subject                              subject
  from                                 from, to, cc, bcc
  date                                 date
  seen                                 seen
  folder                               folder
  has_attachments (bool)               attachments (Vec<u8> data)
                                       body_text, body_html
                                       flags, headers
```

- `Vec<EmailSummary>` held in memory for the email list (~100 bytes per email)
- `Email` loaded on-demand only when viewing/replying/forwarding (~10KB-10MB per email)
- `EmailSummary` is **never** saved to database -- it's a read-only view construct
- `has_attachments` computed via SQL `EXISTS` subquery, not by loading attachment data

---

## Performance Design

| Technique | Impact |
|---|---|
| `EmailSummary` struct (no body/attachments) | ~10-100x less RAM for email list |
| `get_recent_email_summaries()` with `EXISTS` subquery | Single query, no attachment data loaded |
| On-demand `get_email_full()` | Body + attachments loaded only when viewing |
| 30s database poll interval | Low CPU when idle |
| Debounced spell/grammar/address parsing | Avoids per-keystroke processing |
| IMAP IDLE | Server push instead of polling |
| Batch fetch (500 emails per batch) | Avoids memory spikes on large folders |
| SQLite indexes on account+folder, uid, date | Fast queries |
| `INSERT OR REPLACE` in transactions | Atomic batch writes |

---

## Security

- Passwords never stored in config file (removed on save)
- Primary: system keyring (GNOME, KDE, macOS Keychain)
- Fallback: XOR-encrypted files with mode 0o600
- TLS/SSL enforced for IMAP and SMTP connections
- Credentials retrieved at client instantiation time only

---

## File Locations

| Path | Purpose |
|---|---|
| `~/.config/tuimail/config.json` | Account and UI configuration |
| `~/.config/tuimail/credentials/*.enc` | Encrypted password fallback |
| `~/.cache/tuimail/emails.db` | Main database (operation queue) |
| `~/.cache/tuimail/{account}/emails.db` | Per-account email cache |
| `/tmp/tuimail_debug.log` | Debug log (when `EMAIL_DEBUG=1`) |

---

## Debugging

```bash
EMAIL_DEBUG=1 tuimail
```

Logs to `/tmp/tuimail_debug.log`:
- IMAP connection establishment/teardown
- IMAP command sequences and responses
- IDLE session lifecycle
- Email parsing details (subject, from, attachment count)
- Database operations
- Sync strategy decisions
- Error stack traces
