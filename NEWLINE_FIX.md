# Newline Rendering Fix for TUImail Compose Mode

## Problem
The email compose mode was not properly rendering newline characters in the body text. When users typed multi-line text, it would appear as a single line with newlines ignored.

## Root Cause
The issue was in the `render_compose_mode` function in `src/ui.rs`. Specifically, in the body text rendering section (around lines 578-591), the code was using:

```rust
Line::from(display_text).into()
```

and 

```rust
Line::from(content.to_string()).into()
```

The `Line::from()` function treats the entire text as a single line, which ignores newline characters (`\n`). This caused multi-line text to be displayed as a single line.

## Solution
The fix involved creating two new helper functions that properly handle newlines by splitting text into multiple lines:

### 1. `create_text_with_cursor(text: &str, cursor_pos: usize) -> Text<'static>`
- Splits text by newline characters (`\n`)
- Processes each line separately
- Correctly positions the cursor on the appropriate line
- Returns a `Text` object with multiple `Line` objects

### 2. `create_text_without_cursor(text: &str) -> Text<'static>`
- Splits text by newline characters (`\n`)
- Converts each line to a `Line` object
- Returns a `Text` object with multiple `Line` objects

## Implementation Details

The fix replaces the problematic code in the body text rendering section:

**Before:**
```rust
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
```

**After:**
```rust
} else if app.compose_field == crate::app::ComposeField::Body {
    // Just add cursor without spell highlighting
    let cursor_pos = app.compose_cursor_pos.min(content.len());
    create_text_with_cursor(content, cursor_pos)
} else {
    // Plain text without cursor
    create_text_without_cursor(content)
};
```

## Key Features of the Fix

1. **Preserves Newlines**: Text is properly split into multiple lines
2. **Cursor Positioning**: Cursor is correctly positioned on the appropriate line
3. **Character Counting**: Properly handles character offsets across multiple lines
4. **Consistent with Existing Code**: Uses the same approach as the `create_highlighted_text` function
5. **No Breaking Changes**: The fix is backward compatible

## Testing
The fix has been tested with:
- Multi-line text input
- Cursor positioning across different lines
- Character counting with newlines
- Build verification (compiles successfully)

## Files Modified
- `src/ui.rs`: Updated body text rendering logic and added helper functions

## Result
Users can now properly compose multi-line emails in TUImail, with newlines correctly displayed and cursor positioning working across multiple lines.
