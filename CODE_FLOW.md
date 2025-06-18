# TUImail Code Flow Documentation

## Detailed Function Call Flows

This document traces the exact function calls for major operations in TUImail, providing a detailed map for developers to understand how data flows through the system.

## 1. Application Startup Flow

### Complete Startup Sequence
```
main.rs::main()
├── clap::Command::parse() → Parse CLI arguments
├── match args.command:
│   ├── Some(Subcommand::AddAccount) → handle_add_account()
│   └── None → run_tui_mode()
│
run_tui_mode()
├── config::Config::load_or_create() → Load ~/.config/tuimail/config.json
├── credentials::SecureCredentials::new() → Initialize keyring
├── crossterm::terminal::enable_raw_mode()
├── crossterm::execute!(stdout(), EnterAlternateScreen)
├── ratatui::Terminal::new(CrosstermBackend::new(stdout()))
├── app::App::new(config, credentials)
├── run_app(&mut terminal, &mut app)
└── cleanup: disable_raw_mode(), LeaveAlternateScreen
```

### App Initialization Detail
```
app::App::new(config, credentials)
├── Create empty HashMap<usize, AccountData>
├── Set current_account_idx = 0
├── Initialize UI state (mode: Normal, focus: EmailList)
├── Create empty vectors for emails, folders, etc.
└── Return App instance

app::App::init()
├── if config.accounts.is_empty() → return error
├── for (idx, account) in config.accounts.iter().enumerate():
│   ├── init_account(idx)
│   │   ├── EmailClient::new(account.clone(), credentials.clone())
│   │   ├── accounts.insert(idx, AccountData::new())
│   │   ├── load_folders_for_account(idx)
│   │   │   ├── client.list_folders()
│   │   │   │   ├── connect_imap_secure() or connect_imap_plain()
│   │   │   │   ├── session.list(None, Some("*"))
│   │   │   │   └── parse folder names
│   │   │   └── account_data.folders = folders
│   │   └── load_emails_for_account_folder(idx, "INBOX")
│   └── if idx == current_account_idx → set as current
├── rebuild_folder_items() → Build UI folder tree
└── start_background_email_fetching(current_account_idx, "INBOX")
```

## 2. Main Event Loop Flow

### Event Loop Detail
```
main.rs::run_app(terminal, app)
├── app.init() → Initialize accounts and data
├── start_background_email_fetching() → Spawn IDLE thread
├── loop {
│   ├── app.check_for_new_emails()
│   │   ├── if let Some(receiver) = &self.email_receiver:
│   │   ├── match receiver.try_recv():
│   │   │   ├── Ok(new_emails) → self.emails = new_emails
│   │   │   ├── Err(Empty) → continue (normal)
│   │   │   └── Err(Disconnected) → cleanup receiver
│   │   └── update account cache and UI selection
│   │
│   ├── terminal.draw(|frame| ui::ui(frame, app))
│   │   ├── ui::render_main_layout()
│   │   ├── match app.mode:
│   │   │   ├── Normal → render_email_list() + render_folders()
│   │   │   ├── EmailView → render_email_content()
│   │   │   ├── Compose → render_compose_form()
│   │   │   └── FileBrowser → render_file_browser()
│   │   └── render_status_bar() + render_help()
│   │
│   ├── crossterm::event::poll(Duration::from_millis(100))
│   ├── if event available:
│   │   ├── crossterm::event::read() → KeyEvent
│   │   └── app.handle_key_event(key)
│   │       ├── match app.mode:
│   │       │   ├── Normal → handle_normal_mode()
│   │       │   ├── EmailView → handle_email_view_mode()
│   │       │   ├── Compose → handle_compose_mode()
│   │       │   └── FileBrowser → handle_file_browser_mode()
│   │       └── update app state based on key
│   │
│   ├── app.tick() → Update timers, cleanup, etc.
│   └── if app.should_quit → break
│ }
└── app.stop_background_email_fetching() → Cleanup
```

## 3. Email Operations Flow

### Fetching Emails (Refresh)
```
User presses 'r' key:

app::handle_normal_mode(KeyCode::Char('r'))
├── load_emails_for_selected_folder()
│   ├── get_selected_folder_info() → (account_idx, folder_path)
│   └── load_emails_for_account_folder(account_idx, folder_path)
│       ├── ensure_account_initialized(account_idx)
│       ├── accounts.get_mut(&account_idx).email_client
│       └── client.fetch_emails(folder, 50)
│           ├── connect_imap_secure() or connect_imap_plain()
│           │   ├── TlsConnector::new() (if SSL)
│           │   ├── TcpStream::connect(server:port)
│           │   ├── connector.connect(domain, stream) (if SSL)
│           │   └── imap::Client::new(stream).login(user, pass)
│           │
│           ├── session.select(folder)
│           ├── session.search("ALL") → get message UIDs
│           ├── for each UID in recent messages:
│           │   ├── session.fetch(uid, "ENVELOPE BODY.PEEK[]")
│           │   ├── mail_parser::Message::parse(raw_email)
│           │   ├── extract_email_info() → Email struct
│           │   │   ├── parse subject, from, to, date
│           │   │   ├── extract_attachments()
│           │   │   ├── extract_body_content()
│           │   │   └── determine read/unread status
│           │   └── add to emails vector
│           │
│           ├── cache_emails(folder, emails) → save to ~/.cache/tuimail/
│           └── return emails vector
│
├── self.emails = fetched_emails
├── self.selected_email_idx = Some(0) if not empty
└── show_info("Emails refreshed")
```

### Sending Email
```
User presses 'c' (compose):

app::handle_normal_mode(KeyCode::Char('c'))
├── mode = AppMode::Compose
├── compose_to = String::new()
├── compose_subject = String::new()
├── compose_body = String::new()
└── focus = FocusPanel::ComposeForm

User fills form and presses Ctrl+S:

app::handle_compose_mode(KeyCode::Char('s') + Ctrl)
├── validate_compose_fields()
│   ├── check compose_to is not empty
│   ├── validate email address format
│   └── return validation result
│
├── send_email()
│   ├── get current account config
│   ├── email::EmailClient::send_email()
│   │   ├── create_smtp_transport()
│   │   │   ├── match smtp_security:
│   │   │   │   ├── SSL → SmtpTransport::relay_ssl()
│   │   │   │   ├── StartTLS → SmtpTransport::starttls_relay()
│   │   │   │   └── None → SmtpTransport::builder_dangerous()
│   │   │   ├── credentials.get_smtp_password()
│   │   │   └── transport.credentials(Credentials::new(user, pass))
│   │   │
│   │   ├── lettre::Message::builder()
│   │   │   ├── .from(account.email.parse()?)
│   │   │   ├── .to(compose_to.parse()?)
│   │   │   ├── .subject(compose_subject)
│   │   │   └── .body(compose_body)
│   │   │
│   │   ├── if attachments exist:
│   │   │   ├── create MultiPart::mixed()
│   │   │   ├── add text body as SinglePart
│   │   │   └── for each attachment:
│   │   │       ├── read file content
│   │   │       ├── detect MIME type
│   │   │       └── add as Attachment
│   │   │
│   │   └── transport.send(&message)
│   │
│   ├── match send result:
│   │   ├── Ok(_) → show_info("Email sent successfully")
│   │   └── Err(e) → show_error(&format!("Failed to send: {}", e))
│   │
│   └── mode = AppMode::Normal (return to email list)
```

### Mark Email as Read
```
User presses Enter on email:

app::handle_normal_mode(KeyCode::Enter)
├── if let Some(idx) = selected_email_idx:
│   ├── mode = AppMode::EmailView
│   ├── focus = FocusPanel::EmailContent
│   └── mark_current_email_as_read()
│       ├── let email = &emails[idx]
│       ├── if !email.read → proceed with marking
│       └── email::EmailClient::mark_as_read(email)
│           ├── debug_log("Marking email as read: {}")
│           ├── let mut attempts = 0; max_attempts = 3
│           ├── while attempts < max_attempts:
│           │   ├── attempts += 1
│           │   ├── match account.imap_security:
│           │   │   ├── SSL/StartTLS → connect_imap_secure()
│           │   │   └── None → connect_imap_plain()
│           │   │
│           │   ├── session.select(&email.folder)
│           │   ├── session.store(&email.id, "+FLAGS (\\Seen)")
│           │   ├── match result:
│           │   │   ├── Ok(_) → return Ok(()) (success)
│           │   │   └── Err(e) → log error, maybe retry
│           │   │
│           │   └── if failed and attempts < max:
│           │       └── sleep(500ms) then retry
│           │
│           └── return final result
│
├── update email.read = true in local cache
└── trigger UI refresh
```

## 4. Background IDLE Processing Flow

### Starting IDLE Session
```
app::start_background_email_fetching(account_idx, folder)
├── stop_background_email_fetching() → cleanup existing
├── accounts.get(&account_idx).email_client
├── if client.supports_idle():
│   ├── debug_log("Starting background email fetching with IDLE")
│   ├── let (tx, rx) = std::sync::mpsc::channel()
│   ├── let running = Arc::new(Mutex::new(true))
│   ├── clone client, folder, running for thread
│   │
│   ├── std::thread::spawn(move || {
│   │   ├── client_clone.run_idle_session(&folder_clone, &tx, &running_clone)
│   │   └── debug_log on error
│   │ })
│   │
│   ├── self.email_receiver = Some(rx)
│   └── self.fetcher_running = Some(running)
│
└── else: debug_log("Server does not support IDLE")
```

### IDLE Session Loop
```
email::EmailClient::run_idle_session(folder, tx, running)
├── match account.imap_security:
│   ├── SSL/StartTLS → run_idle_session_secure()
│   └── None → run_idle_session_plain()
│
run_idle_session_secure(folder, tx, running):
├── connect_imap_secure() → establish connection
├── session.select(folder) → select folder
├── session.capabilities() → check server capabilities
├── if !caps.has_str("IDLE") → return error
├── debug_log("Server supports IDLE, starting loop")
│
├── loop {
│   ├── check running flag:
│   │   ├── let running_guard = running.lock().unwrap()
│   │   └── if !*running_guard → break (stop requested)
│   │
│   ├── session.idle() → start IDLE command
│   ├── match idle_handle.wait_with_timeout(30s):
│   │   ├── Ok(_) → server sent notification
│   │   │   ├── debug_log("received notification, fetching emails")
│   │   │   ├── fetch_emails(folder, 50) → get updated emails
│   │   │   ├── match tx.send(emails):
│   │   │   │   ├── Ok(_) → debug_log("emails sent to UI")
│   │   │   │   └── Err(_) → debug_log("channel closed"), return
│   │   │   └── continue loop
│   │   │
│   │   └── Err(_) → timeout (normal), continue loop
│   │
│   └── on IDLE error → sleep(30s), continue loop
│ }
│
└── debug_log("IDLE session stopped")
```

### Processing IDLE Updates
```
app::check_for_new_emails() (called every 100ms in main loop)
├── if let Some(receiver) = &self.email_receiver:
│   ├── match receiver.try_recv():
│   │   ├── Ok(new_emails) →
│   │   │   ├── debug_log("Received {} new emails from background")
│   │   │   ├── self.emails = new_emails
│   │   │   ├── update account cache:
│   │   │   │   └── accounts.get_mut(&current_account_idx).emails = emails.clone()
│   │   │   ├── reset selection if needed:
│   │   │   │   └── if selected_email_idx.is_none() && !emails.empty() → Some(0)
│   │   │   └── show_info("New emails received")
│   │   │
│   │   ├── Err(TryRecvError::Empty) → no new emails (normal)
│   │   │
│   │   └── Err(TryRecvError::Disconnected) →
│   │       ├── debug_log("Background email fetcher disconnected")
│   │       ├── self.email_receiver = None
│   │       └── self.fetcher_running = None
│   │
│   └── UI automatically refreshes on next draw cycle
```

## 5. File Operations Flow

### Saving Attachments
```
User presses 's' on attachment in email view:

app::handle_email_view_mode(KeyCode::Char('s'))
├── if let Some(email) = get_current_email():
│   ├── if !email.attachments.is_empty():
│   │   ├── let attachment = &email.attachments[selected_attachment_idx]
│   │   ├── file_browser_save_data = attachment.content.clone()
│   │   ├── mode = AppMode::FileBrowser
│   │   ├── file_browser_mode = FileBrowserMode::Save
│   │   └── init_file_browser(Some(attachment.filename))
│   │       ├── file_browser_current_dir = env::current_dir()
│   │       ├── file_browser_filename = attachment.filename
│   │       ├── load_directory_contents(current_dir)
│   │       │   ├── fs::read_dir(path)
│   │       │   ├── for each entry:
│   │       │   │   ├── get metadata (is_dir, size, modified)
│   │       │   │   └── create FileItem { name, is_dir, size, ... }
│   │       │   ├── sort: directories first, then files alphabetically
│   │       │   └── file_browser_items = items
│   │       └── file_browser_selected = 0
│   │
│   └── else: show_error("No attachments to save")
```

### File Browser Navigation
```
User navigates in file browser:

app::handle_file_browser_mode(key)
├── match key.code:
│   ├── KeyCode::Up → 
│   │   └── file_browser_selected = max(0, selected - 1)
│   │
│   ├── KeyCode::Down →
│   │   └── file_browser_selected = min(items.len()-1, selected + 1)
│   │
│   ├── KeyCode::Enter →
│   │   ├── let item = &file_browser_items[file_browser_selected]
│   │   ├── if item.is_dir:
│   │   │   ├── file_browser_current_dir = current_dir.join(&item.name)
│   │   │   ├── load_directory_contents(new_dir)
│   │   │   └── file_browser_selected = 0
│   │   │
│   │   └── else: (file selected)
│   │       ├── if mode == Save:
│   │       │   ├── file_browser_filename = item.name.clone()
│   │       │   └── file_browser_editing_filename = true
│   │       └── else: (load mode) → load file
│   │
│   ├── KeyCode::Char('f') → (edit filename)
│   │   ├── file_browser_editing_filename = true
│   │   └── focus on filename input
│   │
│   ├── KeyCode::Char('s') → (save with current filename)
│   │   └── save_attachment()
│   │       ├── let full_path = current_dir.join(&filename)
│   │       ├── fs::write(full_path, &file_browser_save_data)
│   │       ├── match result:
│   │       │   ├── Ok(_) → show_info("Attachment saved")
│   │       │   └── Err(e) → show_error("Failed to save: {}")
│   │       ├── mode = AppMode::EmailView
│   │       └── clear file browser state
│   │
│   └── KeyCode::Esc → cancel, return to previous mode
```

## 6. Account Management Flow

### Switching Accounts
```
User presses 'n' (next account):

app::handle_normal_mode(KeyCode::Char('n'))
├── rotate_to_next_account()
│   ├── if config.accounts.len() <= 1:
│   │   └── show_info("Only one account configured"), return
│   │
│   ├── next_account_idx = (current_account_idx + 1) % accounts.len()
│   ├── current_account_idx = next_account_idx
│   ├── ensure_account_initialized(next_account_idx)
│   │   ├── if accounts.contains_key(&account_idx) → already initialized
│   │   └── else: init_account(account_idx) → full initialization
│   │
│   ├── check if need to load emails:
│   │   ├── if let Some(account_data) = accounts.get(&next_account_idx):
│   │   │   └── need_to_load = account_data.emails.is_empty()
│   │   └── else: need_to_load = true
│   │
│   ├── if need_to_load_emails:
│   │   └── load_emails_for_account_folder(next_account_idx, "INBOX")
│   │ else:
│   │   ├── self.emails = account_data.emails.clone() (use cache)
│   │   └── show_info("Switched to account: {}")
│   │
│   ├── selected_email_idx = if emails.empty() { None } else { Some(0) }
│   ├── ensure_account_expanded(next_account_idx) → expand in folder view
│   ├── rebuild_folder_items() → update folder tree UI
│   ├── select_inbox_folder_for_account(next_account_idx)
│   └── start_background_email_fetching(next_account_idx, "INBOX")
│       ├── stop_background_email_fetching() → stop current IDLE
│       └── start new IDLE session for new account
```

### Adding New Account
```
Command: tuimail add-account

main.rs::handle_add_account()
├── println!("Adding new email account...")
├── prompt_for_input("Account name") → get name
├── prompt_for_input("Email address") → get email, validate format
├── prompt_for_input("IMAP server") → get imap_server
├── prompt_for_input("IMAP port") → get imap_port, parse as u16
├── prompt_for_input("IMAP security") → get security (SSL/StartTLS/None)
├── prompt_for_input("IMAP username") → get imap_username
├── rpassword::prompt_password("IMAP password") → get imap_password
├── prompt_for_input("SMTP server") → get smtp_server
├── prompt_for_input("SMTP port") → get smtp_port, parse as u16
├── prompt_for_input("SMTP security") → get smtp_security
├── prompt_for_input("SMTP username") → get smtp_username
├── rpassword::prompt_password("SMTP password") → get smtp_password
│
├── create EmailAccount struct with all details
├── test connections:
│   ├── println!("Testing IMAP connection...")
│   ├── EmailClient::new(account.clone(), credentials.clone())
│   ├── client.test_imap_connection()
│   │   ├── connect_imap_secure() or connect_imap_plain()
│   │   ├── session.capabilities() → verify connection works
│   │   └── return Ok(()) or Err(error)
│   │
│   ├── println!("Testing SMTP connection...")
│   └── client.test_smtp_connection()
│       ├── create_smtp_transport()
│       ├── transport.test_connection() → verify SMTP works
│       └── return Ok(()) or Err(error)
│
├── store credentials securely:
│   ├── account.store_imap_password(&credentials, &imap_password)
│   │   ├── credentials.store_password(&account_id, "imap", &password)
│   │   └── keyring or fallback encrypted storage
│   └── account.store_smtp_password(&credentials, &smtp_password)
│
├── add to config:
│   ├── config.accounts.push(account)
│   ├── if config.accounts.len() == 1 → set as default
│   └── config.save() → write to ~/.config/tuimail/config.json
│
└── println!("Account added successfully!")
```

This detailed flow documentation provides a complete picture of how data moves through TUImail, making it easier for developers to understand, debug, and extend the codebase.
