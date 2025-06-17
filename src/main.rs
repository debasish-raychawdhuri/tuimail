mod app;
mod config;
mod credentials;
mod email;
mod ui;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use log::error;
use ratatui::prelude::*;

use crate::app::{App, AppResult, AppError};
use crate::config::{Config, EmailAccount, ImapSecurity, SmtpSecurity};
use crate::credentials::SecureCredentials;
use crate::ui::ui;

/// Terminal-based email client with IMAP and SMTP support
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to config file
    #[clap(short, long, default_value = "~/.config/email_client/config.json")]
    config: String,
    
    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Add a new email account
    AddAccount {
        /// Account name
        #[clap(short, long)]
        name: String,
        
        /// Email address
        #[clap(short, long)]
        email: String,
        
        /// IMAP server address
        #[clap(long)]
        imap_server: String,
        
        /// IMAP server port
        #[clap(long, default_value = "993")]
        imap_port: u16,
        
        /// IMAP security (None, StartTLS, SSL)
        #[clap(long, default_value = "SSL")]
        imap_security: String,
        
        /// IMAP username
        #[clap(long)]
        imap_username: String,
        
        /// IMAP password
        #[clap(long)]
        imap_password: String,
        
        /// SMTP server address
        #[clap(long)]
        smtp_server: String,
        
        /// SMTP server port
        #[clap(long, default_value = "587")]
        smtp_port: u16,
        
        /// SMTP security (None, StartTLS, SSL)
        #[clap(long, default_value = "StartTLS")]
        smtp_security: String,
        
        /// SMTP username
        #[clap(long)]
        smtp_username: String,
        
        /// SMTP password
        #[clap(long)]
        smtp_password: String,
    },
    
    /// List configured accounts
    ListAccounts,
    
    /// Set default account
    SetDefaultAccount {
        /// Account index (starting from 0)
        #[clap(short, long)]
        index: usize,
    },
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize debug logging early if EMAIL_DEBUG is set
    if std::env::var("EMAIL_DEBUG").is_ok() {
        let log_file = "/tmp/email_client_debug.log";
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_file) 
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] Email client starting with debug logging", 
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }
    
    // Initialize logger
    env_logger::Builder::new()
        .filter_level(if args.debug { log::LevelFilter::Debug } else { log::LevelFilter::Info })
        .init();
    
    // Load configuration
    let config_path = shellexpand::tilde(&args.config).into_owned();
    let mut config = Config::load(&config_path).unwrap_or_else(|_| {
        println!("No config found at {}. Creating default config.", config_path);
        Config::default()
    });
    
    // Handle subcommands
    if let Some(cmd) = args.command {
        match cmd {
            Commands::AddAccount {
                name,
                email,
                imap_server,
                imap_port,
                imap_security,
                imap_username,
                imap_password,
                smtp_server,
                smtp_port,
                smtp_security,
                smtp_username,
                smtp_password,
            } => {
                // Initialize secure credential storage
                let credentials = SecureCredentials::new()
                    .context("Failed to initialize secure credential storage")?;

                // Parse security settings
                let imap_security = match imap_security.to_lowercase().as_str() {
                    "none" => ImapSecurity::None,
                    "starttls" => ImapSecurity::StartTLS,
                    "ssl" => ImapSecurity::SSL,
                    _ => {
                        println!("Invalid IMAP security setting. Using SSL.");
                        ImapSecurity::SSL
                    }
                };
                
                let smtp_security = match smtp_security.to_lowercase().as_str() {
                    "none" => SmtpSecurity::None,
                    "starttls" => SmtpSecurity::StartTLS,
                    "ssl" => SmtpSecurity::SSL,
                    _ => {
                        println!("Invalid SMTP security setting. Using StartTLS.");
                        SmtpSecurity::StartTLS
                    }
                };
                
                // Create account (without passwords in config)
                let account = EmailAccount {
                    name,
                    email: email.clone(),
                    imap_server,
                    imap_port,
                    imap_security,
                    imap_username,
                    smtp_server,
                    smtp_port,
                    smtp_security,
                    smtp_username,
                    signature: Some("Sent from Email Client".to_string()),
                };

                // Store passwords securely
                account.store_imap_password(&credentials, &imap_password)
                    .context("Failed to store IMAP password securely")?;
                account.store_smtp_password(&credentials, &smtp_password)
                    .context("Failed to store SMTP password securely")?;
                
                // Add account to config
                config.accounts.push(account);
                
                // If this is the first account, set it as default
                if config.accounts.len() == 1 {
                    config.default_account = 0;
                }
                
                // Save config
                if let Err(e) = config.save(&config_path) {
                    println!("Failed to save config: {}", e);
                    return Ok(());
                }
                
                println!("✓ Account added successfully with secure password storage!");
                return Ok(());
            }
            Commands::ListAccounts => {
                println!("Configured accounts:");
                for (i, account) in config.accounts.iter().enumerate() {
                    println!("{}. {} <{}> ({})", 
                        i, 
                        account.name, 
                        account.email,
                        if i == config.default_account { "default" } else { "" }
                    );
                }
                return Ok(());
            }
            Commands::SetDefaultAccount { index } => {
                if index >= config.accounts.len() {
                    println!("Error: Account index out of bounds");
                    return Ok(());
                }
                
                config.default_account = index;
                
                // Save config
                if let Err(e) = config.save(&config_path) {
                    println!("Failed to save config: {}", e);
                    return Ok(());
                }
                
                println!("Default account set to: {} <{}>", 
                    config.accounts[index].name, 
                    config.accounts[index].email
                );
                return Ok(());
            }
        }
    }
    
    // Check if we have any accounts configured
    if config.accounts.is_empty() {
        println!("No email accounts configured. Please add an account first:");
        println!("  email_client add-account --help");
        return Ok(());
    }
    
    // Save config in case it was created for the first time
    if let Err(e) = config.save(&config_path) {
        println!("Failed to save config: {}", e);
    }
    
    // Check if we need to migrate passwords from old config format BEFORE entering TUI mode
    if let Err(e) = migrate_passwords_if_needed(&mut config, &config_path) {
        println!("Warning: Failed to migrate passwords to secure storage: {}", e);
        println!("You may need to re-add your accounts with secure password storage.");
    }
    
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    io::stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .context("Failed to create terminal")?;
    
    // Clear the terminal to ensure clean start
    terminal.clear().context("Failed to clear terminal")?;
    
    // Create app state
    let mut app = App::new(config);
    
    // Debug logging
    if std::env::var("EMAIL_DEBUG").is_ok() {
        let log_file = "/tmp/email_client_debug.log";
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(log_file) 
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] App created, about to call run_app", 
                Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }
    
    // Run the application
    let result = run_app(&mut terminal, &mut app);
    
    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    io::stdout()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    
    // If there was an error, print it
    if let Err(err) = result {
        error!("Error: {:?}", err);
        println!("Error: {:?}", err);
    }
    
    Ok(())
}

/// Migrate passwords from old config format to secure storage
fn migrate_passwords_if_needed(config: &mut Config, config_path: &str) -> Result<()> {
    // Check if any account has passwords in the config (old format)
    let mut needs_migration = false;
    let mut accounts_to_migrate = Vec::new();
    
    // Read the raw config file to check for password fields
    if let Ok(config_content) = std::fs::read_to_string(config_path) {
        if config_content.contains("imap_password") || config_content.contains("smtp_password") {
            needs_migration = true;
            
            // Parse the old format to extract passwords
            if let Ok(old_config_value) = serde_json::from_str::<serde_json::Value>(&config_content) {
                if let Some(accounts) = old_config_value["accounts"].as_array() {
                    for (i, account) in accounts.iter().enumerate() {
                        if let (Some(email), Some(imap_pass), Some(smtp_pass)) = (
                            account["email"].as_str(),
                            account["imap_password"].as_str(),
                            account["smtp_password"].as_str(),
                        ) {
                            accounts_to_migrate.push((i, email.to_string(), imap_pass.to_string(), smtp_pass.to_string()));
                        }
                    }
                }
            }
        }
    }
    
    if needs_migration && !accounts_to_migrate.is_empty() {
        println!("🔐 Migrating passwords to secure storage...");
        
        let credentials = SecureCredentials::new()
            .context("Failed to initialize secure credential storage")?;
        
        for (i, email, imap_password, smtp_password) in accounts_to_migrate {
            if i < config.accounts.len() {
                let account = &config.accounts[i];
                
                // Store passwords securely
                account.store_imap_password(&credentials, &imap_password)
                    .context(format!("Failed to store IMAP password for {}", email))?;
                account.store_smtp_password(&credentials, &smtp_password)
                    .context(format!("Failed to store SMTP password for {}", email))?;
                
                println!("✓ Migrated passwords for {}", email);
            }
        }
        
        // Save the updated config without passwords
        config.save(config_path)
            .context("Failed to save updated config after migration")?;
        
        println!("✓ Password migration completed successfully!");
        println!("  Passwords are now stored securely in your system keyring.");
    }
    
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> AppResult<()> {
    // Initialize app with error handling
    if let Err(e) = app.init() {
        // Log the error to debug file if debug is enabled
        if std::env::var("EMAIL_DEBUG").is_ok() {
            let log_file = "/tmp/email_client_debug.log";
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(log_file) 
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] App initialization failed: {}", 
                    Local::now().format("%Y-%m-%d %H:%M:%S"), e);
            }
        }
        return Err(e);
    }
    
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;
    
    loop {
        // Draw UI
        if let Err(e) = terminal.draw(|frame| ui(frame, app)) {
            consecutive_errors += 1;
            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                return Err(AppError::IoError(e));
            }
            continue;
        }
        
        // Ensure the terminal output is flushed
        io::stdout().flush().ok();
        
        // Reset consecutive error counter on successful draw
        consecutive_errors = 0;
        
        // Handle events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle input with error recovery
                    if let Err(e) = app.handle_key_event(key) {
                        app.show_error(&format!("Error: {}", e));
                        consecutive_errors += 1;
                        
                        // If we have too many consecutive errors, exit
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            return Err(e);
                        }
                    } else {
                        // Reset error counter on successful operation
                        consecutive_errors = 0;
                    }
                    
                    // Check if we should exit
                    if app.should_quit {
                        return Ok(());
                    }
                }
            }
        }
        
        // Update app state with error handling
        if let Err(e) = app.tick() {
            app.show_error(&format!("Update error: {}", e));
            consecutive_errors += 1;
            
            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                return Err(e);
            }
        } else {
            // Reset error counter on successful tick
            if consecutive_errors > 0 {
                consecutive_errors = 0;
            }
        }
    }
}
