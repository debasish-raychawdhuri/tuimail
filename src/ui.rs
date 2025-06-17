use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::app::{App, AppMode};
use crate::email::Email;

pub fn ui(f: &mut Frame, app: &App) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.size());

    render_title_bar(f, app, chunks[0]);
    render_main_content(f, app, chunks[1]);
    render_status_bar(f, app, chunks[2]);
}

fn render_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["Inbox", "Compose", "Settings", "Help"];
    let tabs = Tabs::new(titles.iter().cloned().map(Line::from).collect())
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(Style::default().fg(Color::Yellow))
        .select(match app.mode {
            AppMode::Normal | AppMode::ViewEmail | AppMode::FolderList => 0,
            AppMode::Compose => 1,
            AppMode::AccountSettings => 2,
            AppMode::Help => 3,
        });
    f.render_widget(tabs, area);
}

fn render_main_content(f: &mut Frame, app: &App, area: Rect) {
    match app.mode {
        AppMode::Normal => render_normal_mode(f, app, area),
        AppMode::ViewEmail => render_view_email_mode(f, app, area),
        AppMode::Compose => render_compose_mode(f, app, area),
        AppMode::FolderList => render_folder_list_mode(f, app, area),
        AppMode::AccountSettings => render_settings_mode(f, app, area),
        AppMode::Help => render_help_mode(f, app, area),
    }
}

fn render_normal_mode(f: &mut Frame, app: &App, area: Rect) {
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Folder list
            Constraint::Percentage(80), // Email list
        ])
        .split(area);

    render_folder_list(f, app, horizontal_chunks[0]);
    render_email_list(f, app, horizontal_chunks[1]);
}

fn render_folder_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .folder_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (text, style) = match item {
                crate::app::FolderItem::Account { name, email, expanded, .. } => {
                    let prefix = if *expanded { "▼ " } else { "▶ " };
                    let display_text = format!("{}{} <{}>", prefix, name, email);
                    let style = if i == app.selected_folder_item_idx {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    };
                    (display_text, style)
                }
                crate::app::FolderItem::Folder { name, .. } => {
                    let display_text = format!("  📁 {}", name);
                    let style = if i == app.selected_folder_item_idx {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    (display_text, style)
                }
            };
            
            ListItem::new(text).style(style)
        })
        .collect();

    let folders = List::new(items)
        .block(Block::default().title("Accounts & Folders").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    // Add scrolling support
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.selected_folder_item_idx));

    f.render_stateful_widget(folders, area, &mut state);
}

fn render_email_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .emails
        .iter()
        .enumerate()
        .map(|(i, email)| {
            let style = if Some(i) == app.selected_email_idx {
                Style::default().fg(Color::Yellow)
            } else if !email.seen {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            
            let date = email.date.format("%m-%d %H:%M").to_string();
            let from = email.from.first().map_or("Unknown", |addr| {
                // Show name if available, otherwise show email address
                if let Some(ref name) = addr.name {
                    if !name.is_empty() {
                        name
                    } else {
                        &addr.address
                    }
                } else {
                    &addr.address
                }
            });
            
            let content = format!("{:<12} {:<25} {}", date, from, email.subject);
            ListItem::new(content).style(style)
        })
        .collect();

    let emails = List::new(items)
        .block(Block::default().title("Emails").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    // Add scrolling support
    let mut state = ratatui::widgets::ListState::default();
    if let Some(selected) = app.selected_email_idx {
        state.select(Some(selected));
    }

    f.render_stateful_widget(emails, area, &mut state);
}

fn render_view_email_mode(f: &mut Frame, app: &App, area: Rect) {
    if let Some(idx) = app.selected_email_idx {
        if idx < app.emails.len() {
            let email = &app.emails[idx];
            
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Header (increased for better display)
                    Constraint::Min(0),    // Body
                ])
                .split(area);
            
            render_email_header(f, email, chunks[0]);
            render_scrollable_email_body(f, email, chunks[1], app.email_view_scroll);
        }
    }
}

fn render_scrollable_email_body(f: &mut Frame, email: &Email, area: Rect, scroll_offset: usize) {
    let content = email.body_text.as_deref().unwrap_or("No content");
    
    let body = Paragraph::new(content)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Body (↑/↓ to scroll, PgUp/PgDn for fast scroll)"))
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));
    
    f.render_widget(body, area);
}

fn render_email_header(f: &mut Frame, email: &Email, area: Rect) {
    let from = email.from.first().map_or("Unknown", |addr| {
        addr.name.as_deref().unwrap_or(&addr.address)
    });
    
    let to = email.to.iter()
        .map(|addr| addr.address.clone())
        .collect::<Vec<_>>()
        .join(", ");
    
    let header_text = vec![
        Line::from(vec![
            Span::styled("From: ", Style::default().fg(Color::Gray)),
            Span::raw(from),
        ]),
        Line::from(vec![
            Span::styled("To: ", Style::default().fg(Color::Gray)),
            Span::raw(to),
        ]),
        Line::from(vec![
            Span::styled("Subject: ", Style::default().fg(Color::Gray)),
            Span::raw(&email.subject),
        ]),
        Line::from(vec![
            Span::styled("Date: ", Style::default().fg(Color::Gray)),
            Span::raw(email.date.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().title("Email").borders(Borders::ALL));
    
    f.render_widget(header, area);
}

fn render_email_body(f: &mut Frame, email: &Email, area: Rect) {
    let content = email.body_text.as_deref().unwrap_or("No content");
    
    let body = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((0, 0)); // Add scroll support
    
    f.render_widget(body, area);
}

fn render_compose_mode(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Header fields (increased for better display)
            Constraint::Min(0),    // Body
        ])
        .split(area);
    
    // Render compose form header with field highlighting
    let to_display = if app.compose_field == crate::app::ComposeField::To {
        // Show cursor in To field when active
        let cursor_pos = app.compose_cursor_pos.min(app.compose_to_text.len());
        let mut display_text = app.compose_to_text.clone();
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│'); // Vertical bar as cursor
        }
        display_text
    } else {
        app.compose_to_text.clone()
    };
    
    let to_style = if app.compose_field == crate::app::ComposeField::To {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let subject_style = if app.compose_field == crate::app::ComposeField::Subject {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let header_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("To: ", to_style),
            Span::raw(&to_display),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Subject: ", subject_style),
            Span::raw(&app.compose_email.subject),
        ]),
        Line::from(""),
        Line::from("Tab/↑↓: Navigate fields | Ctrl+S: Send | Esc: Cancel"),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().title("New Email").borders(Borders::ALL));
    
    f.render_widget(header, chunks[0]);
    
    // Render compose form body with highlighting and cursor
    let content = app.compose_email.body_text.as_deref().unwrap_or("");
    
    let body_style = if app.compose_field == crate::app::ComposeField::Body {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    
    let body_title = if app.compose_field == crate::app::ComposeField::Body {
        "Body (Active - Type to edit, ←→ to move cursor)"
    } else {
        "Body"
    };
    
    // If we're in the body field, show cursor by inserting a cursor character
    let display_content = if app.compose_field == crate::app::ComposeField::Body {
        let cursor_pos = app.compose_cursor_pos.min(content.len());
        let mut display_text = content.to_string();
        
        // Insert cursor character at the cursor position
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│'); // Vertical bar as cursor
        }
        display_text
    } else {
        content.to_string()
    };
    
    let body = Paragraph::new(display_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(body_title)
            .border_style(body_style))
        .style(body_style)
        .wrap(Wrap { trim: false });
    
    f.render_widget(body, chunks[1]);
}

fn render_folder_list_mode(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .folder_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (text, style) = match item {
                crate::app::FolderItem::Account { name, email, expanded, .. } => {
                    let prefix = if *expanded { "▼ " } else { "▶ " };
                    let display_text = format!("{}{} <{}>", prefix, name, email);
                    let style = if i == app.selected_folder_item_idx {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    };
                    (display_text, style)
                }
                crate::app::FolderItem::Folder { name, .. } => {
                    let display_text = format!("  📁 {}", name);
                    let style = if i == app.selected_folder_item_idx {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    (display_text, style)
                }
            };
            
            ListItem::new(text).style(style)
        })
        .collect();

    let folders = List::new(items)
        .block(Block::default()
            .title("Select Account or Folder (↑/↓: Navigate, Enter: Select/Expand, Esc: Cancel)")
            .borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    // Add scrolling support
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.selected_folder_item_idx));

    // Center the folder list
    let centered_area = centered_rect(80, 80, area);
    f.render_stateful_widget(folders, centered_area, &mut state);
}

fn render_settings_mode(f: &mut Frame, app: &App, area: Rect) {
    let account = app.config.get_current_account_safe();
    
    let settings_text = vec![
        Line::from(vec![
            Span::styled("Account Name: ", Style::default().fg(Color::Gray)),
            Span::raw(&account.name),
        ]),
        Line::from(vec![
            Span::styled("Email: ", Style::default().fg(Color::Gray)),
            Span::raw(&account.email),
        ]),
        Line::from(vec![
            Span::styled("IMAP Server: ", Style::default().fg(Color::Gray)),
            Span::raw(&account.imap_server),
        ]),
        Line::from(vec![
            Span::styled("SMTP Server: ", Style::default().fg(Color::Gray)),
            Span::raw(&account.smtp_server),
        ]),
    ];
    
    let settings = Paragraph::new(settings_text)
        .block(Block::default().title("Account Settings").borders(Borders::ALL));
    
    // Center the settings
    let centered_area = centered_rect(60, 80, area);
    f.render_widget(settings, centered_area);
}

fn render_help_mode(f: &mut Frame, _app: &App, area: Rect) {
    let help_text = vec![
        Line::from("Email Client Help"),
        Line::from(""),
        Line::from("Global:"),
        Line::from("  q - Quit (in normal mode)"),
        Line::from("  ? - Show/hide help"),
        Line::from(""),
        Line::from("Normal Mode:"),
        Line::from("  c - Compose new email"),
        Line::from("  r - Refresh emails"),
        Line::from("  f - Show folder list"),
        Line::from("  s - Show settings"),
        Line::from("  ↑/↓ - Navigate emails"),
        Line::from("  Enter - View selected email"),
        Line::from("  Delete - Delete selected email"),
        Line::from(""),
        Line::from("View Email Mode:"),
        Line::from("  Esc - Return to email list"),
        Line::from("  r - Reply to email"),
        Line::from("  a - Reply to all"),
        Line::from("  f - Forward email"),
        Line::from("  d - Delete email"),
        Line::from("  ↑↓ - Scroll email content"),
        Line::from(""),
        Line::from("Compose Mode:"),
        Line::from("  Esc - Cancel"),
        Line::from("  Ctrl+s - Send email"),
    ];
    
    let help = Paragraph::new(help_text)
        .block(Block::default().title("Help").borders(Borders::ALL));
    
    // Center the help text
    let centered_area = centered_rect(60, 80, area);
    f.render_widget(help, centered_area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut text = String::new();
    
    // Show current account and folder
    if let Some(account_data) = app.accounts.get(&app.current_account_idx) {
        if !account_data.folders.is_empty() {
            text.push_str(&format!("Folder: {} | ", account_data.folders[account_data.selected_folder_idx]));
        }
    }
    
    // Show email count
    text.push_str(&format!("Emails: {} | ", app.emails.len()));
    
    // Show sync status
    if app.is_syncing {
        text.push_str("Syncing... | ");
    } else if let Some(last_sync) = app.last_sync {
        text.push_str(&format!("Last sync: {} | ", last_sync.format("%H:%M:%S")));
    }
    
    // Show current mode and help
    match app.mode {
        AppMode::Normal => text.push_str("Press 'r' to refresh, 'f' for folders, 'c' to compose, '?' for help"),
        AppMode::FolderList => text.push_str("Use ↑↓ to navigate folders, Enter to select, Esc to cancel"),
        AppMode::Compose => text.push_str("Tab to switch fields, Ctrl+S to send, Esc to cancel"),
        AppMode::ViewEmail => text.push_str("r=Reply, a=Reply All, f=Forward, d=Delete, ↑↓=Scroll, Esc=Back"),
        _ => text.push_str(&format!("Mode: {:?}", app.mode)),
    }
    
    // Show error or info message if present (override other text)
    if let Some(error) = &app.error_message {
        text = format!("ERROR: {}", error);
    } else if let Some(info) = &app.info_message {
        text = format!("INFO: {}", info);
    }
    
    let status = Paragraph::new(text)
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    
    f.render_widget(status, area);
}

// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
