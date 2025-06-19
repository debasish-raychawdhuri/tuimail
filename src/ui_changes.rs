// Updated create_highlighted_text function
fn create_highlighted_text(
    text: &str, 
    spell_errors: &[crate::spellcheck::SpellError], 
    grammar_errors: &[crate::grammarcheck::GrammarError],
    show_cursor: bool, 
    cursor_pos: usize
) -> ratatui::text::Text<'static> {
    let mut spans = Vec::new();
    let mut last_pos = 0;
    
    // Create a combined list of all errors with their positions and styles
    let mut all_errors = Vec::new();
    
    // Add spell errors
    for error in spell_errors {
        all_errors.push((
            error.position,
            error.position + error.word.len(),
            Style::default().bg(Color::Red).fg(Color::White)
        ));
    }
    
    // Add grammar errors
    for error in grammar_errors {
        all_errors.push((
            error.start,
            error.end,
            Style::default().bg(Color::Blue).fg(Color::White)
        ));
    }
    
    // Sort all errors by position
    all_errors.sort_by_key(|e| e.0);
    
    // Process each error and create styled spans
    for (start, end, style) in all_errors {
        // Add normal text before the error
        if start > last_pos {
            let normal_text = text[last_pos..start].to_string();
            spans.push(Span::raw(normal_text));
        }
        
        // Skip if this error overlaps with text we've already processed
        if start < last_pos {
            continue;
        }
        
        // Add the error text with appropriate style
        let error_text = text[start..end].to_string();
        spans.push(Span::styled(error_text, style));
        
        last_pos = end;
    }
    
    // Add remaining text after the last error
    if last_pos < text.len() {
        spans.push(Span::raw(text[last_pos..].to_string()));
    }
    
    // If showing cursor, insert it at the cursor position
    if show_cursor {
        let mut final_spans = Vec::new();
        let mut cursor_inserted = false;
        let mut current_pos = 0;
        
        for span in spans {
            let span_text = span.content.to_string();
            let span_style = span.style;
            let span_len = span_text.len();
            
            // If cursor is in this span
            if !cursor_inserted && current_pos <= cursor_pos && current_pos + span_len >= cursor_pos {
                let cursor_offset = cursor_pos - current_pos;
                
                // Text before cursor
                if cursor_offset > 0 {
                    final_spans.push(Span::styled(
                        span_text[..cursor_offset].to_string(),
                        span_style,
                    ));
                }
                
                // Cursor
                final_spans.push(Span::styled(
                    "█".to_string(),
                    Style::default().fg(Color::Black).bg(Color::White),
                ));
                
                // Text after cursor
                if cursor_offset < span_len {
                    final_spans.push(Span::styled(
                        span_text[cursor_offset..].to_string(),
                        span_style,
                    ));
                }
                
                cursor_inserted = true;
            } else {
                final_spans.push(span);
            }
            
            current_pos += span_len;
        }
        
        // If cursor is at the end of text
        if !cursor_inserted && cursor_pos >= text.len() {
            final_spans.push(Span::styled(
                "█".to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ));
        }
        
        spans = final_spans;
    }
    
    ratatui::text::Text::from(ratatui::text::Line::from(spans))
}

// Grammar check status rendering
fn render_grammar_check_status(f: &mut Frame, app: &App, area: Rect) {
    let status_text = if app.grammar_check_enabled {
        if let Some(stats) = app.get_grammar_stats() {
            if stats.error_count > 0 {
                format!(
                    "Grammar Check: {} errors | Alt+R: Toggle | Alt+T: Suggestions | Quality: {:.1}%",
                    stats.error_count,
                    stats.quality_score
                )
            } else {
                "Grammar Check: No errors | Alt+R: Toggle | Alt+T: Suggestions".to_string()
            }
        } else {
            "Grammar Check: Enabled | Alt+R: Toggle | Alt+T: Suggestions".to_string()
        }
    } else {
        "Grammar Check: Disabled | Alt+R: Enable".to_string()
    };

    let status_color = if app.grammar_check_enabled {
        if app.grammar_errors.is_empty() {
            Color::Green
        } else {
            Color::Blue
        }
    } else {
        Color::Gray
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color))
        .alignment(Alignment::Left);

    f.render_widget(status, area);
}

// Grammar suggestions popup
fn render_grammar_suggestions(f: &mut Frame, app: &App, area: Rect) {
    // Find the current error at cursor position
    let mut current_error: Option<&crate::grammarcheck::GrammarError> = None;
    for error in &app.grammar_errors {
        if app.compose_cursor_pos >= error.start && app.compose_cursor_pos <= error.end {
            current_error = Some(error);
            break;
        }
    }

    if let Some(error) = current_error {
        // Create a popup in the center of the screen
        let popup_area = centered_rect(60, 60, area);
        
        // Clear the background
        let clear = Block::default().style(Style::default().bg(Color::Black));
        f.render_widget(clear, area);
        
        // Create a block for the popup
        let block = Block::default()
            .title("Grammar Suggestions")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Blue));
        
        f.render_widget(block, popup_area);
        
        // Create inner area for content
        let inner_area = Rect {
            x: popup_area.x + 2,
            y: popup_area.y + 2,
            width: popup_area.width - 4,
            height: popup_area.height - 4,
        };
        
        // Create layout for content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Error message
                Constraint::Length(1), // Spacer
                Constraint::Min(3),    // Suggestions list
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Instructions
            ])
            .split(inner_area);
        
        // Show error message
        let error_text = format!("Grammar issue: {}", error.message);
        let error_paragraph = Paragraph::new(error_text)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true });
        
        f.render_widget(error_paragraph, chunks[0]);
        
        // Show suggestions
        let mut items = Vec::new();
        for (i, suggestion) in error.replacements.iter().enumerate() {
            let style = if i == app.selected_grammar_suggestion {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            
            items.push(ListItem::new(suggestion.clone()).style(style));
        }
        
        let suggestions_list = List::new(items)
            .block(Block::default().title("Suggestions").borders(Borders::ALL))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        
        f.render_widget(suggestions_list, chunks[2]);
        
        // Show instructions
        let instructions = "↑↓: Navigate suggestions | Enter: Apply | Esc: Cancel";
        let instructions_paragraph = Paragraph::new(instructions)
            .style(Style::default().fg(Color::Gray));
        
        f.render_widget(instructions_paragraph, chunks[4]);
    }
}
