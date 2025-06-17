mod app;
mod config;
mod email;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use log::error;
use ratatui::prelude::*;

use crate::app::{App, AppResult, AppError};
use crate::config::{Config, EmailAccount, ImapSecurity, SmtpSecurity};
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
                
                // Create account
                let account = EmailAccount {
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
                    signature: Some("Sent from Email Client".to_string()),
                };
                
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
                
                println!("Account added successfully!");
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
    
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    io::stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .context("Failed to create terminal")?;
    
    // Create app state
    let mut app = App::new(config);
    
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> AppResult<()> {
    // Initialize app with error handling
    if let Err(e) = app.init() {
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
