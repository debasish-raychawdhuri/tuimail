# Async Grammar Checking Implementation

## Overview
This document describes the implementation of asynchronous grammar checking in TUImail, which activates only after 2 seconds of typing inactivity to improve performance and user experience.

## Problem Solved
The previous synchronous grammar checking had several issues:
- **Performance Impact**: Grammar checking on every keystroke caused UI lag
- **Resource Intensive**: Continuous grammar checking consumed unnecessary CPU
- **Poor UX**: Users experienced delays while typing due to blocking grammar checks

## Solution: Async Grammar Checking with Delay

### Key Features
1. **2-Second Delay**: Grammar checking only triggers after 2 seconds of typing inactivity
2. **Non-blocking**: Grammar checking runs in background without blocking UI
3. **Cancellation**: New typing cancels pending grammar checks
4. **Request Deduplication**: Only the most recent grammar check request is processed

### Architecture

#### Components

1. **AsyncGrammarChecker** (`src/async_grammar.rs`)
   - Main async grammar checker struct
   - Manages background task and message passing
   - Handles request queuing and cancellation

2. **Background Task**
   - Runs in separate tokio task
   - Processes grammar check requests with 2-second delay
   - Performs actual grammar checking using nlprule

3. **Message System**
   - Uses tokio mpsc channels for communication
   - Request messages: `CheckText`, `Cancel`, `Shutdown`
   - Response messages: `GrammarCheckResponse`

#### Message Flow
```
User Types → App::request_grammar_check() → AsyncGrammarChecker::request_check()
                                                    ↓
Background Task ← Message Queue ← GrammarCheckMessage::CheckText
                                                    ↓
                                            Wait 2 seconds
                                                    ↓
                                          Perform Grammar Check
                                                    ↓
Response Queue ← GrammarCheckResponse ← Background Task
                                                    ↓
App::process_grammar_responses() ← Main Event Loop
                                                    ↓
                                          Update UI with Results
```

### Implementation Details

#### AsyncGrammarChecker Structure
```rust
pub struct AsyncGrammarChecker {
    sender: mpsc::UnboundedSender<GrammarCheckMessage>,
    response_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<GrammarCheckResponse>>>,
    next_request_id: Arc<std::sync::atomic::AtomicU64>,
}
```

#### Key Methods
- `request_check(text, field_type)`: Request grammar check with 2-second delay
- `cancel_pending()`: Cancel any pending grammar checks
- `try_receive_response()`: Non-blocking check for completed grammar checks
- `background_task()`: Main async task that processes requests

#### App Integration
- **App struct**: Added `async_grammar_checker` and `last_grammar_request_id` fields
- **Event handling**: Replaced `check_grammar()` calls with `request_grammar_check()`
- **Main loop**: Added `process_grammar_responses().await` to handle completed checks

### Performance Benefits

#### Before (Synchronous)
- Grammar check on every keystroke: ~50-100ms per check
- UI blocking during grammar analysis
- Continuous CPU usage while typing

#### After (Asynchronous)
- Grammar check only after 2 seconds of inactivity
- Non-blocking UI during typing
- Reduced CPU usage (checks only when needed)
- Better typing experience

### Usage

#### For Users
- **No change in UI**: Grammar checking works the same from user perspective
- **Better performance**: Smoother typing experience
- **Same shortcuts**: `Alt+R` to toggle, `Alt+T` for suggestions

#### For Developers
```rust
// Request async grammar check
app.request_grammar_check();

// Process responses in main loop
app.process_grammar_responses().await;

// Cancel pending checks
if let Some(ref checker) = app.async_grammar_checker {
    checker.cancel_pending();
}
```

### Configuration
- **Delay**: 2 seconds (hardcoded in `async_grammar.rs`)
- **Cancellation**: Automatic on new typing
- **Fields**: Subject and Body only (email addresses skipped)

### Error Handling
- **Initialization failures**: Graceful fallback (no grammar checking)
- **Channel errors**: Logged but don't crash the application
- **Grammar check failures**: Logged and ignored

### Testing
The async grammar checker can be tested by:
1. Starting TUImail in compose mode
2. Typing text and observing no immediate grammar checking
3. Stopping typing for 2+ seconds
4. Observing grammar errors appear after the delay

### Future Enhancements
- **Configurable delay**: Allow users to set custom delay time
- **Smart delay**: Shorter delay for longer pauses, longer for rapid typing
- **Background caching**: Cache grammar results for repeated text
- **Progressive checking**: Check sentences as they're completed

### Files Modified
- `src/async_grammar.rs`: New async grammar checker implementation
- `src/app.rs`: Updated to use async grammar checker
- `src/main.rs`: Made main function async, added response processing
- `Cargo.toml`: Already had tokio dependency

### Dependencies
- **tokio**: For async runtime and channels
- **nlprule**: For actual grammar checking (unchanged)
- **anyhow**: For error handling

## Benefits Summary
✅ **Better Performance**: No UI blocking during typing
✅ **Reduced CPU Usage**: Grammar checks only when needed
✅ **Improved UX**: Smoother typing experience
✅ **Smart Cancellation**: New typing cancels old checks
✅ **Non-breaking**: Same user interface and shortcuts
✅ **Robust**: Proper error handling and graceful degradation
