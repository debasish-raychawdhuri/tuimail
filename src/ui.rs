use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
    // Get current account name for display
    let current_account_name = if app.current_account_idx < app.config.accounts.len() {
        &app.config.accounts[app.current_account_idx].name
    } else {
        "Unknown"
    };
    
    let inbox_title = if app.config.accounts.len() > 1 {
        format!("Inbox ({})", current_account_name)
    } else {
        "Inbox".to_string()
    };
    
    let titles = vec![inbox_title.as_str(), "Compose", "Settings", "Help"];
    let tabs = Tabs::new(titles.iter().cloned().map(Line::from).collect())
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(Style::default().fg(Color::Yellow))
        .select(match app.mode {
            AppMode::Normal | AppMode::ViewEmail | AppMode::FolderList | AppMode::DeleteConfirm => 0,
            AppMode::Compose => 1,
            AppMode::AccountSettings => 2,
            AppMode::Help => 3,
        });
    f.render_widget(tabs, area);
}

fn render_main_content(f: &mut Frame, app: &App, area: Rect) {
    // If in file browser mode, show the file browser regardless of current mode
    if app.file_browser_mode {
        render_file_browser(f, app, area);
        return;
    }
    
    match app.mode {
        AppMode::Normal => render_normal_mode(f, app, area),
        AppMode::ViewEmail => render_view_email_mode(f, app, area),
        AppMode::Compose => render_compose_mode(f, app, area),
        AppMode::FolderList => render_folder_list_mode(f, app, area),
        AppMode::AccountSettings => render_settings_mode(f, app, area),
        AppMode::Help => render_help_mode(f, app, area),
        AppMode::DeleteConfirm => render_delete_confirm_mode(f, app, area),
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
            
            let attachment_indicator = if !email.attachments.is_empty() {
                "📎 "
            } else {
                "   " // Three spaces to match the width of "📎 " (emoji takes 2 chars + 1 space)
            };
            
            let content = format!("{}{:<12} {:<25} {}", 
                attachment_indicator, date, from, email.subject);
            ListItem::new(content).style(style)
        })
        .collect();

    // Create title showing current account and folder
    let title = if app.config.accounts.len() > 1 {
        let account_name = if app.current_account_idx < app.config.accounts.len() {
            &app.config.accounts[app.current_account_idx].name
        } else {
            "Unknown"
        };
        format!("Emails - {} (INBOX)", account_name)
    } else {
        "Emails".to_string()
    };

    let emails = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
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
            
            // Determine layout based on whether there are attachments
            let constraints = if email.attachments.is_empty() {
                vec![
                    Constraint::Length(6), // Header
                    Constraint::Min(0),    // Body
                ]
            } else {
                vec![
                    Constraint::Length(6), // Header
                    Constraint::Length(4 + email.attachments.len().min(5) as u16), // Attachments (max 5 visible)
                    Constraint::Min(0),    // Body
                ]
            };
            
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);
            
            render_email_header(f, email, chunks[0]);
            
            if !email.attachments.is_empty() {
                render_email_attachments(f, app, email, chunks[1]);
                render_scrollable_email_body(f, email, chunks[2], app.email_view_scroll);
            } else {
                render_scrollable_email_body(f, email, chunks[1], app.email_view_scroll);
            }
        }
    }
}

fn render_email_attachments(f: &mut Frame, app: &App, email: &Email, area: Rect) {
    let items: Vec<ListItem> = email
        .attachments
        .iter()
        .enumerate()
        .map(|(i, attachment)| {
            let size = format_file_size(attachment.data.len());
            let style = if Some(i) == app.selected_attachment_idx {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            
            let content = format!("📎 {} ({}) - {}", 
                attachment.filename, 
                size, 
                attachment.content_type
            );
            ListItem::new(content).style(style)
        })
        .collect();

    let attachments = List::new(items)
        .block(Block::default()
            .title("Attachments (Tab to select, 's' to save)")
            .borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ratatui::widgets::ListState::default();
    if let Some(selected) = app.selected_attachment_idx {
        state.select(Some(selected));
    }

    f.render_stateful_widget(attachments, area, &mut state);
}

fn format_file_size(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
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

#[allow(dead_code)]
fn render_email_body(f: &mut Frame, email: &Email, area: Rect) {
    let content = email.body_text.as_deref().unwrap_or("No content");
    
    let body = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((0, 0)); // Add scroll support
    
    f.render_widget(body, area);
}

fn render_compose_mode(f: &mut Frame, app: &App, area: Rect) {
    // If showing spell suggestions, render the suggestion popup
    if app.show_spell_suggestions {
        render_spell_suggestions(f, app, area);
        return;
    }
    
    // If showing grammar suggestions, render the suggestion popup
    if app.show_grammar_suggestions {
        render_grammar_suggestions(f, app, area);
        return;
    }
    
    // If in attachment input mode, show the input dialog
    if app.attachment_input_mode {
        render_attachment_input_dialog(f, app, area);
        return;
    }
    
    // Determine layout based on whether there are attachments
    let constraints = if app.compose_email.attachments.is_empty() {
        vec![
            Constraint::Length(12), // Header fields (To, CC, BCC, Subject)
            Constraint::Min(0),     // Body
            Constraint::Length(2),  // Status area (spell + grammar check)
        ]
    } else {
        vec![
            Constraint::Length(12), // Header fields (To, CC, BCC, Subject)
            Constraint::Length(4 + app.compose_email.attachments.len().min(3) as u16), // Attachments (max 3 visible)
            Constraint::Min(0),     // Body
            Constraint::Length(2),  // Status area (spell + grammar check)
        ]
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
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
    
    let cc_display = if app.compose_field == crate::app::ComposeField::Cc {
        // Show cursor in CC field when active
        let cursor_pos = app.compose_cursor_pos.min(app.compose_cc_text.len());
        let mut display_text = app.compose_cc_text.clone();
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│'); // Vertical bar as cursor
        }
        display_text
    } else {
        app.compose_cc_text.clone()
    };
    
    let bcc_display = if app.compose_field == crate::app::ComposeField::Bcc {
        // Show cursor in BCC field when active
        let cursor_pos = app.compose_cursor_pos.min(app.compose_bcc_text.len());
        let mut display_text = app.compose_bcc_text.clone();
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│'); // Vertical bar as cursor
        }
        display_text
    } else {
        app.compose_bcc_text.clone()
    };
    
    let to_style = if app.compose_field == crate::app::ComposeField::To {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let cc_style = if app.compose_field == crate::app::ComposeField::Cc {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let bcc_style = if app.compose_field == crate::app::ComposeField::Bcc {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let subject_style = if app.compose_field == crate::app::ComposeField::Subject {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    // Create subject text with spell checking if enabled
    let subject_text = if app.spell_check_enabled && app.compose_field == crate::app::ComposeField::Subject {
        // Filter errors that are in the subject field
        let subject_errors: Vec<_> = app.spell_errors.iter()
            .filter(|_| {
                // In subject field, we need to check if the error is in the subject
                // This is a simplification - in a real app you'd track field-specific errors
                app.compose_field == crate::app::ComposeField::Subject
            })
            .cloned()
            .collect();
        // Filter grammar errors for subject field
        let subject_grammar_errors: Vec<crate::grammarcheck::GrammarError> = app.grammar_errors
            .iter()
            .filter(|error| {
                // Check if this grammar error is in the subject field
                // For now, we'll assume grammar errors in subject are those with small positions
                // This is a simplification - ideally we'd track which field each error belongs to
                error.start < app.compose_email.subject.len()
            })
            .cloned()
            .collect();
        
        if !subject_errors.is_empty() || !subject_grammar_errors.is_empty() {
            create_highlighted_text(&app.compose_email.subject, &subject_errors, &subject_grammar_errors, true, app.compose_cursor_pos)
        } else if app.compose_field == crate::app::ComposeField::Subject {
            // Just add cursor without highlighting
            let cursor_pos = app.compose_cursor_pos.min(app.compose_email.subject.len());
            let mut display_text = app.compose_email.subject.clone();
            if cursor_pos <= display_text.len() {
                display_text.insert(cursor_pos, '│');
            }
            Line::from(display_text).into()
        } else {
            Line::from(app.compose_email.subject.clone()).into()
        }
    } else if app.compose_field == crate::app::ComposeField::Subject {
        // Just add cursor without spell highlighting
        let cursor_pos = app.compose_cursor_pos.min(app.compose_email.subject.len());
        let mut display_text = app.compose_email.subject.clone();
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│');
        }
        Line::from(display_text).into()
    } else {
        Line::from(app.compose_email.subject.clone()).into()
    };
    
    let header_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("To: ", to_style),
            Span::raw(&to_display),
        ]),
        Line::from(vec![
            Span::styled("CC: ", cc_style),
            Span::raw(&cc_display),
        ]),
        Line::from(vec![
            Span::styled("BCC: ", bcc_style),
            Span::raw(&bcc_display),
        ]),
        Line::from(vec![
            Span::styled("Subject: ", subject_style),
        ]),
        // Add the subject text with potential highlighting
        // We can't directly use Line::from(subject_text) because subject_text is already a Text
        Line::from(""),
        Line::from("Tab/↑↓: Navigate fields | Ctrl+S: Send | Esc: Cancel"),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().title("New Email").borders(Borders::ALL));
    
    f.render_widget(header, chunks[0]);
    
    // Render subject text separately if it has highlighting
    if app.spell_check_enabled && app.compose_field == crate::app::ComposeField::Subject {
        let subject_area = Rect {
            x: chunks[0].x + 10, // Offset to align with "Subject: " text
            y: chunks[0].y + 6,  // Position after the "Subject: " line (adjusted for CC/BCC)
            width: chunks[0].width - 12,
            height: 1,
        };
        
        let subject_para = Paragraph::new(subject_text);
        f.render_widget(subject_para, subject_area);
    }
    
    // Render attachments if any
    let body_chunk_idx = if app.compose_email.attachments.is_empty() {
        1
    } else {
        render_compose_attachments(f, app, chunks[1]);
        2
    };
    
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
    // and highlight misspelled words and grammar errors
    let body_content = if (app.spell_check_enabled && !app.spell_errors.is_empty()) || (app.grammar_check_enabled && !app.grammar_errors.is_empty()) {
        // Filter grammar errors for body field (those beyond subject length)
        let body_grammar_errors: Vec<crate::grammarcheck::GrammarError> = app.grammar_errors
            .iter()
            .filter(|error| {
                // Grammar errors in body field would have positions beyond subject
                error.start >= app.compose_email.subject.len()
            })
            .map(|error| {
                // Adjust positions relative to body start
                let mut adjusted_error = error.clone();
                adjusted_error.start = adjusted_error.start.saturating_sub(app.compose_email.subject.len());
                adjusted_error.end = adjusted_error.end.saturating_sub(app.compose_email.subject.len());
                adjusted_error
            })
            .collect();
            
        // Create styled spans with misspelled words and grammar errors highlighted
        let styled_content = create_highlighted_text(content, &app.spell_errors, &body_grammar_errors, app.compose_field == crate::app::ComposeField::Body, app.compose_cursor_pos);
        styled_content
    } else if app.compose_field == crate::app::ComposeField::Body {
        // Just add cursor without spell highlighting
        let cursor_pos = app.compose_cursor_pos.min(content.len());
        let mut display_text = content.to_string();
        
        // Insert cursor character at the cursor position
        if cursor_pos <= display_text.len() {
            display_text.insert(cursor_pos, '│'); // Vertical bar as cursor
        }
        
        // Convert to Text for rendering
        Line::from(display_text).into()
    } else {
        // Plain text without cursor
        Line::from(content.to_string()).into()
    };
    
    let body = Paragraph::new(body_content)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(body_title)
            .border_style(body_style))
        .style(body_style)
        .wrap(Wrap { trim: false });
    
    f.render_widget(body, chunks[body_chunk_idx]);
    
    // Render spell check status bar
    let status_chunk_idx = if app.compose_email.attachments.is_empty() {
        2
    } else {
        3
    };
    
    if status_chunk_idx < chunks.len() {
        render_check_status(f, app, chunks[status_chunk_idx]);
    }
}

fn render_check_status(f: &mut Frame, app: &App, area: Rect) {
    // Split the status area into two parts: spell check and grammar check
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spell check status
            Constraint::Length(1), // Grammar check status
        ])
        .split(area);

    // Render spell check status
    let spell_status_text = if app.spell_check_enabled {
        if let Some(stats) = app.get_spell_stats() {
            if stats.misspelled_words > 0 {
                format!(
                    "Spell: {} errors | Alt+S: Toggle | Alt+G: Suggestions | Alt+D: Add to dict | Accuracy: {:.1}%",
                    stats.misspelled_words,
                    stats.accuracy
                )
            } else {
                "Spell: No errors | Alt+S: Toggle | Alt+G: Suggestions | Alt+D: Add to dict".to_string()
            }
        } else {
            "Spell: Enabled | Alt+S: Toggle | Alt+G: Suggestions | Alt+D: Add to dict".to_string()
        }
    } else {
        "Spell: Disabled | Alt+S: Enable".to_string()
    };

    let spell_status_color = if app.spell_check_enabled {
        if app.spell_errors.is_empty() {
            Color::Green
        } else {
            Color::Yellow
        }
    } else {
        Color::Gray
    };

    let spell_status = Paragraph::new(spell_status_text)
        .style(Style::default().fg(spell_status_color))
        .alignment(Alignment::Left);

    f.render_widget(spell_status, status_chunks[0]);

    // Render grammar check status
    let grammar_status_text = if app.grammar_check_enabled {
        if let Some(stats) = app.get_grammar_stats() {
            if stats.error_count > 0 {
                format!(
                    "Grammar: {} errors | Alt+R: Toggle | Alt+T: Suggestions | Quality: {:.1}%",
                    stats.error_count,
                    stats.quality_score
                )
            } else {
                "Grammar: No errors | Alt+R: Toggle | Alt+T: Suggestions".to_string()
            }
        } else {
            "Grammar: Enabled | Alt+R: Toggle | Alt+T: Suggestions".to_string()
        }
    } else {
        "Grammar: Disabled | Alt+R: Enable".to_string()
    };

    let grammar_status_color = if app.grammar_check_enabled {
        if app.grammar_errors.is_empty() {
            Color::Green
        } else {
            Color::Blue
        }
    } else {
        Color::Gray
    };

    let grammar_status = Paragraph::new(grammar_status_text)
        .style(Style::default().fg(grammar_status_color))
        .alignment(Alignment::Left);

    f.render_widget(grammar_status, status_chunks[1]);
}

fn render_spell_suggestions(f: &mut Frame, app: &App, area: Rect) {
    // Find the current error at cursor position
    let mut current_error: Option<&crate::spellcheck::SpellError> = None;
    for error in &app.spell_errors {
        let word_end = error.position + error.word.len();
        if app.compose_cursor_pos >= error.position && app.compose_cursor_pos <= word_end {
            current_error = Some(error);
            break;
        }
    }

    if let Some(error) = current_error {
        // Create a popup in the center of the screen
        let popup_area = centered_rect(50, 60, area);
        
        // Clear the background
        let clear = Block::default().style(Style::default().bg(Color::Black));
        f.render_widget(clear, area);

        // Create suggestion items
        let items: Vec<ListItem> = error.suggestions
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if i == app.selected_spell_suggestion {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(suggestion.as_str()).style(style)
            })
            .collect();

        let suggestions_list = List::new(items)
            .block(Block::default()
                .title(format!("Suggestions for '{}'", error.word))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        f.render_widget(suggestions_list, popup_area);

        // Add help text at the bottom
        let help_area = Rect {
            x: popup_area.x,
            y: popup_area.y + popup_area.height,
            width: popup_area.width,
            height: 1,
        };

        if help_area.y < area.height {
            let help_text = "↑↓: Navigate | Enter: Apply | Esc: Cancel";
            let help = Paragraph::new(help_text)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            f.render_widget(help, help_area);
        }
    }
}

fn render_grammar_suggestions(f: &mut Frame, app: &App, area: Rect) {
    // Find the current grammar error at cursor position
    let mut current_error: Option<&crate::grammarcheck::GrammarError> = None;
    for error in &app.grammar_errors {
        if app.compose_cursor_pos >= error.start && app.compose_cursor_pos <= error.end {
            current_error = Some(error);
            break;
        }
    }

    if let Some(error) = current_error {
        // Create a popup in the center of the screen
        let popup_area = centered_rect(60, 70, area);
        
        // Clear the background
        let clear = Block::default().style(Style::default().bg(Color::Black));
        f.render_widget(clear, area);

        // Create suggestion items
        let items: Vec<ListItem> = error.replacements
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if i == app.selected_grammar_suggestion {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(suggestion.as_str()).style(style)
            })
            .collect();

        // Get the original text for the error
        let original_text = match app.compose_field {
            crate::app::ComposeField::Subject => {
                if error.end <= app.compose_email.subject.len() {
                    &app.compose_email.subject[error.start..error.end]
                } else {
                    "unknown"
                }
            },
            crate::app::ComposeField::Body => {
                if let Some(ref body) = app.compose_email.body_text {
                    if error.end <= body.len() {
                        &body[error.start..error.end]
                    } else {
                        "unknown"
                    }
                } else {
                    "unknown"
                }
            },
            _ => "unknown"
        };

        let suggestions_list = List::new(items)
            .block(Block::default()
                .title(format!("Grammar suggestions for '{}'", original_text))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)))
            .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

        f.render_widget(suggestions_list, popup_area);

        // Add error message and help text at the bottom
        let message_area = Rect {
            x: popup_area.x,
            y: popup_area.y + popup_area.height,
            width: popup_area.width,
            height: 2,
        };

        if message_area.y + 1 < area.height {
            let message_text = format!("Error: {}", error.message);
            let help_text = "↑↓: Navigate | Enter: Apply | Esc: Cancel";
            
            let message = Paragraph::new(vec![
                Line::from(Span::styled(message_text, Style::default().fg(Color::Yellow))),
                Line::from(Span::styled(help_text, Style::default().fg(Color::Gray))),
            ])
            .alignment(Alignment::Center);
            f.render_widget(message, message_area);
        }
    }
}

fn render_file_browser(f: &mut Frame, app: &App, area: Rect) {
    // Create a centered file browser
    let browser_area = centered_rect(80, 80, area);
    
    // Clear the background
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, area);
    
    // Create file list items
    let items: Vec<ListItem> = app
        .file_browser_items
        .iter()
        .enumerate()
        .map(|(_i, item)| {
            // Don't apply selection styling here - let the List widget handle it
            let style = if item.is_directory {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            
            let icon = if item.is_directory {
                "📁"
            } else {
                "📄"
            };
            
            let size_str = if let Some(size) = item.size {
                format!(" ({})", format_file_size(size as usize))
            } else {
                String::new()
            };
            
            let content = format!("{} {}{}", icon, item.name, size_str);
            ListItem::new(content).style(style)
        })
        .collect();
    
    // Create the file browser title with current path
    let current_path = app.file_browser_current_path.to_string_lossy();
    let title = if app.file_browser_save_mode {
        if app.file_browser_editing_filename {
            format!("Save as: {} - {}", app.file_browser_save_filename, current_path)
        } else {
            format!("Save '{}' - {}", app.file_browser_save_filename, current_path)
        }
    } else {
        format!("File Browser - {}", current_path)
    };
    
    let file_list = List::new(items)
        .block(Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD));
    
    // Create help text
    let help_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),     // File list
            Constraint::Length(3),  // Help text
        ])
        .split(browser_area);
    
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.file_browser_selected));
    
    f.render_stateful_widget(file_list, help_area[0], &mut state);
    
    // Render help text
    let help_text = if app.file_browser_save_mode {
        if app.file_browser_editing_filename {
            vec![
                Line::from("Type filename | Enter: Save | Esc: Cancel editing"),
            ]
        } else {
            vec![
                Line::from("↑↓: Navigate | Enter: Select/Edit | 'f': Edit filename | 's': Save | 'q': Quick Save | Esc: Cancel"),
            ]
        }
    } else {
        vec![
            Line::from("↑↓: Navigate | Enter: Select/Open | Backspace: Parent Dir | Esc: Cancel"),
        ]
    };
    
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::TOP))
        .style(Style::default().fg(Color::Gray));
    
    f.render_widget(help, help_area[1]);
}

fn render_attachment_input_dialog(f: &mut Frame, app: &App, area: Rect) {
    // Create a centered dialog for file path input
    let dialog_area = centered_rect(60, 20, area);
    
    // Clear the background
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, area);
    
    // Create the input dialog
    let cursor_pos = app.attachment_input_text.len();
    
    // Add cursor indicator
    let display_text = if cursor_pos < app.attachment_input_text.len() {
        format!("{}│{}", 
            &app.attachment_input_text[..cursor_pos],
            &app.attachment_input_text[cursor_pos..])
    } else {
        format!("{}│", app.attachment_input_text)
    };
    
    let dialog_content = vec![
        Line::from("Add Attachment"),
        Line::from(""),
        Line::from(format!("File path: {}", display_text)),
        Line::from(""),
        Line::from("Tab - Auto-complete ~/Downloads/"),
        Line::from("Enter - Add attachment"),
        Line::from("Esc - Cancel"),
    ];
    
    let dialog = Paragraph::new(dialog_content)
        .block(Block::default()
            .title("Add Attachment")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)))
        .style(Style::default().fg(Color::White));
    
    f.render_widget(dialog, dialog_area);
}

fn render_compose_attachments(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .compose_email
        .attachments
        .iter()
        .enumerate()
        .map(|(i, attachment)| {
            let size = format_file_size(attachment.data.len());
            let style = if Some(i) == app.selected_attachment_idx {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };
            
            let content = format!("📎 {} ({}) - {}", 
                attachment.filename, 
                size, 
                attachment.content_type
            );
            ListItem::new(content).style(style)
        })
        .collect();

    let attachments = List::new(items)
        .block(Block::default()
            .title("Attachments (Ctrl+A to add, Ctrl+X to remove)")
            .borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut state = ratatui::widgets::ListState::default();
    if let Some(selected) = app.selected_attachment_idx {
        state.select(Some(selected));
    }

    f.render_stateful_widget(attachments, area, &mut state);
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
        Line::from("  n - Next account (rotate)"),
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
        Line::from("  s - Save selected attachment"),
        Line::from("  Tab - Select next attachment"),
        Line::from("  ↑↓ - Scroll email content"),
        Line::from(""),
        Line::from("Compose Mode:"),
        Line::from("  Esc - Cancel"),
        Line::from("  Ctrl+s - Send email"),
        Line::from("  Ctrl+a - Add attachment (file browser)"),
        Line::from("  Ctrl+x - Remove selected attachment"),
        Line::from("  Tab - Switch between fields"),
    ];
    
    let help = Paragraph::new(help_text)
        .block(Block::default().title("Help").borders(Borders::ALL));
    
    // Center the help text
    let centered_area = centered_rect(60, 80, area);
    f.render_widget(help, centered_area);
}

fn render_delete_confirm_mode(f: &mut Frame, app: &App, area: Rect) {
    // First render the normal mode in the background
    render_normal_mode(f, app, area);
    
    // Create the confirmation dialog
    let dialog_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("⚠️  Delete Email Confirmation", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from("Are you sure you want to delete this email?"),
        Line::from("This action cannot be undone."),
        Line::from(""),
        Line::from("Press 'y' to confirm deletion"),
        Line::from("Press 'n' or Esc to cancel"),
        Line::from(""),
    ];
    
    let dialog = Paragraph::new(dialog_text)
        .block(
            Block::default()
                .title("Confirm Delete")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .style(Style::default().bg(Color::DarkGray))
        )
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::DarkGray));
    
    // Center the dialog on screen
    let dialog_area = centered_rect(50, 30, area);
    
    // Render the dialog with solid background
    f.render_widget(dialog, dialog_area);
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
    
    // Add account info if multiple accounts
    if app.config.accounts.len() > 1 {
        let account_name = if app.current_account_idx < app.config.accounts.len() {
            &app.config.accounts[app.current_account_idx].name
        } else {
            "Unknown"
        };
        text.push_str(&format!("Account: {} ({}/{}) | ", 
            account_name,
            app.current_account_idx + 1, 
            app.config.accounts.len()));
    }
    
    // Show sync status
    if app.is_syncing {
        text.push_str("Syncing... | ");
    } else if let Some(last_sync) = app.last_sync {
        text.push_str(&format!("Last sync: {} | ", last_sync.format("%H:%M:%S")));
    }
    
    // Show current mode and help
    match app.mode {
        AppMode::Normal => text.push_str("Press 'r' to refresh, 'n' for next account, 'f' for folders, 'c' to compose, '?' for help"),
        AppMode::FolderList => text.push_str("Use ↑↓ to navigate folders, Enter to select, Esc to cancel"),
        AppMode::Compose => text.push_str("Tab to switch fields, Ctrl+S to send, Esc to cancel"),
        AppMode::ViewEmail => text.push_str("r=Reply, a=Reply All, f=Forward, d=Delete, ↑↓=Scroll, Esc=Back"),
        AppMode::DeleteConfirm => text.push_str("Delete email? Press 'y' to confirm, 'n' or Esc to cancel"),
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

/// Helper function to safely convert byte position to character position
fn byte_to_char_pos(text: &str, byte_pos: usize) -> usize {
    text.char_indices()
        .position(|(i, _)| i >= byte_pos)
        .unwrap_or(text.chars().count())
}

/// Helper function to safely slice text by character positions
fn safe_char_slice(text: &str, start_char: usize, end_char: usize) -> String {
    text.chars()
        .skip(start_char)
        .take(end_char.saturating_sub(start_char))
        .collect()
}

/// Helper function to safely slice text from character position to end
fn safe_char_slice_from(text: &str, start_char: usize) -> String {
    text.chars()
        .skip(start_char)
        .collect()
}

/// Helper function to create text with highlighted misspelled words and grammar errors
fn create_highlighted_text(text: &str, spell_errors: &[crate::spellcheck::SpellError], grammar_errors: &[crate::grammarcheck::GrammarError], show_cursor: bool, cursor_pos: usize) -> ratatui::text::Text<'static> {
    let mut spans = Vec::new();
    let mut last_pos = 0;
    
    // Collect all errors with their types and positions
    #[derive(Debug, Clone)]
    enum ErrorType {
        Spell(crate::spellcheck::SpellError),
        Grammar(crate::grammarcheck::GrammarError),
    }
    
    let mut all_errors = Vec::new();
    
    // Add spell errors
    for error in spell_errors {
        all_errors.push((error.position, ErrorType::Spell(error.clone())));
    }
    
    // Add grammar errors
    for error in grammar_errors {
        all_errors.push((error.start, ErrorType::Grammar(error.clone())));
    }
    
    // Sort all errors by position
    all_errors.sort_by_key(|(pos, _)| *pos);
    
    // Convert text to character positions for safe processing
    let text_len_chars = text.chars().count();
    
    // Process each error and create styled spans
    for (_, error) in all_errors {
        match error {
            ErrorType::Spell(spell_error) => {
                // Convert byte positions to character positions
                let error_start_char = byte_to_char_pos(text, spell_error.position);
                let error_end_char = error_start_char + spell_error.word.chars().count();
                
                // Bounds checking
                if error_start_char >= text_len_chars {
                    continue;
                }
                
                let error_end_char = error_end_char.min(text_len_chars);
                
                // Add normal text before the error
                if error_start_char > last_pos {
                    let normal_text = safe_char_slice(text, last_pos, error_start_char);
                    spans.push(Span::raw(normal_text));
                }
                
                // Add the misspelled word with red background
                let error_text = safe_char_slice(text, error_start_char, error_end_char);
                spans.push(Span::styled(
                    error_text, 
                    Style::default().bg(Color::Red).fg(Color::White)
                ));
                
                last_pos = error_end_char;
            }
            ErrorType::Grammar(grammar_error) => {
                // Convert byte positions to character positions
                let error_start_char = byte_to_char_pos(text, grammar_error.start);
                let error_end_char = byte_to_char_pos(text, grammar_error.end);
                
                // Bounds checking
                if error_start_char >= text_len_chars {
                    continue;
                }
                
                let error_end_char = error_end_char.min(text_len_chars);
                
                // Add normal text before the error
                if error_start_char > last_pos {
                    let normal_text = safe_char_slice(text, last_pos, error_start_char);
                    spans.push(Span::raw(normal_text));
                }
                
                // Add the grammar error with blue underline
                let error_text = safe_char_slice(text, error_start_char, error_end_char);
                spans.push(Span::styled(
                    error_text, 
                    Style::default().bg(Color::Blue).fg(Color::White)
                ));
                
                last_pos = error_end_char;
            }
        }
    }
    
    // Add remaining text after the last error
    if last_pos < text_len_chars {
        let remaining_text = safe_char_slice_from(text, last_pos);
        spans.push(Span::raw(remaining_text));
    }
    
    // If showing cursor, insert it at the cursor position
    if show_cursor {
        let mut final_spans = Vec::new();
        let mut cursor_inserted = false;
        
        for span in spans {
            if !cursor_inserted && span.content.len() + final_spans.iter().map(|s: &Span| s.content.len()).sum::<usize>() >= cursor_pos {
                // Need to split this span to insert cursor
                let span_start = cursor_pos - final_spans.iter().map(|s: &Span| s.content.len()).sum::<usize>();
                
                if span_start > 0 {
                    // Add text before cursor
                    let before_cursor = span.content.chars().take(span_start).collect::<String>();
                    final_spans.push(Span::styled(before_cursor, span.style));
                }
                
                // Add cursor
                final_spans.push(Span::styled("│".to_string(), Style::default().fg(Color::Yellow)));
                
                // Add text after cursor
                let after_cursor = span.content.chars().skip(span_start).collect::<String>();
                if !after_cursor.is_empty() {
                    final_spans.push(Span::styled(after_cursor, span.style));
                }
                
                cursor_inserted = true;
            } else {
                final_spans.push(span);
            }
        }
        
        // If cursor wasn't inserted yet, add it at the end
        if !cursor_inserted {
            final_spans.push(Span::styled("│".to_string(), Style::default().fg(Color::Yellow)));
        }
        
        ratatui::text::Text::from(Line::from(final_spans))
    } else {
        ratatui::text::Text::from(Line::from(spans))
    }
}
