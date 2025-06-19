use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::grammarcheck::{GrammarChecker, GrammarError, GrammarCheckConfig};

/// Message types for the async grammar checker
#[derive(Debug, Clone)]
pub enum GrammarCheckMessage {
    /// Request to check text after a delay
    CheckText {
        text: String,
        field_type: String,
        request_id: u64,
    },
    /// Cancel any pending checks
    Cancel,
    /// Shutdown the checker
    Shutdown,
}

/// Response from the async grammar checker
#[derive(Debug, Clone)]
pub struct GrammarCheckResponse {
    pub errors: Vec<GrammarError>,
    pub field_type: String,
    pub request_id: u64,
}

/// Async grammar checker that performs checks after a delay
pub struct AsyncGrammarChecker {
    sender: mpsc::UnboundedSender<GrammarCheckMessage>,
    response_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<GrammarCheckResponse>>>,
    next_request_id: Arc<std::sync::atomic::AtomicU64>,
}

impl AsyncGrammarChecker {
    /// Create a new async grammar checker
    pub fn new() -> Result<Self> {
        let (msg_sender, mut msg_receiver) = mpsc::unbounded_channel::<GrammarCheckMessage>();
        let (response_sender, response_receiver) = mpsc::unbounded_channel::<GrammarCheckResponse>();
        
        // Initialize the grammar checker
        let grammar_checker = Arc::new(GrammarChecker::new()?);
        
        // Spawn the background task
        tokio::spawn(async move {
            Self::background_task(grammar_checker, msg_receiver, response_sender).await;
        });
        
        Ok(Self {
            sender: msg_sender,
            response_receiver: Arc::new(tokio::sync::Mutex::new(response_receiver)),
            next_request_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        })
    }
    
    /// Request a grammar check with a 2-second delay
    pub fn request_check(&self, text: String, field_type: String) -> u64 {
        let request_id = self.next_request_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        let message = GrammarCheckMessage::CheckText {
            text,
            field_type,
            request_id,
        };
        
        if let Err(e) = self.sender.send(message) {
            log::error!("Failed to send grammar check request: {}", e);
        }
        
        request_id
    }
    
    /// Cancel any pending checks
    pub fn cancel_pending(&self) {
        if let Err(e) = self.sender.send(GrammarCheckMessage::Cancel) {
            log::error!("Failed to send cancel message: {}", e);
        }
    }
    
    /// Try to receive a grammar check response (non-blocking)
    pub async fn try_receive_response(&self) -> Option<GrammarCheckResponse> {
        let mut receiver = self.response_receiver.lock().await;
        receiver.try_recv().ok()
    }
    
    /// Shutdown the async grammar checker
    pub fn shutdown(&self) {
        if let Err(e) = self.sender.send(GrammarCheckMessage::Shutdown) {
            log::error!("Failed to send shutdown message: {}", e);
        }
    }
    
    /// Background task that handles grammar checking with delays
    async fn background_task(
        grammar_checker: Arc<GrammarChecker>,
        mut msg_receiver: mpsc::UnboundedReceiver<GrammarCheckMessage>,
        response_sender: mpsc::UnboundedSender<GrammarCheckResponse>,
    ) {
        let mut pending_check: Option<(String, String, u64, Instant)> = None;
        let check_delay = Duration::from_secs(2);
        
        loop {
            // Calculate how long to wait
            let wait_duration = if let Some((_, _, _, start_time)) = &pending_check {
                let elapsed = start_time.elapsed();
                if elapsed >= check_delay {
                    Duration::from_millis(0) // Ready to process
                } else {
                    check_delay - elapsed
                }
            } else {
                Duration::from_secs(3600) // Wait indefinitely if no pending check
            };
            
            // Wait for either a message or timeout
            let message = if wait_duration.is_zero() {
                // Process pending check immediately
                None
            } else {
                tokio::select! {
                    msg = msg_receiver.recv() => msg,
                    _ = sleep(wait_duration) => None,
                }
            };
            
            match message {
                Some(GrammarCheckMessage::CheckText { text, field_type, request_id }) => {
                    log::debug!("Received grammar check request for {}: '{}'", field_type, text);
                    // Store the new check request, replacing any existing one
                    pending_check = Some((text, field_type, request_id, Instant::now()));
                }
                Some(GrammarCheckMessage::Cancel) => {
                    log::debug!("Cancelling pending grammar checks");
                    pending_check = None;
                }
                Some(GrammarCheckMessage::Shutdown) => {
                    log::info!("Shutting down async grammar checker");
                    break;
                }
                None => {
                    // Timeout occurred, process pending check if any
                    if let Some((text, field_type, request_id, start_time)) = pending_check.take() {
                        if start_time.elapsed() >= check_delay {
                            log::debug!("Processing delayed grammar check for {}", field_type);
                            
                            // Skip grammar check for email address fields
                            if field_type == "To" || field_type == "Cc" || field_type == "Bcc" {
                                log::debug!("Skipping grammar check for email address field: {}", field_type);
                                continue;
                            }
                            
                            // Perform the grammar check
                            let config = GrammarCheckConfig::default();
                            let errors = grammar_checker.check_text(&text, &config);
                            
                            log::debug!("Grammar check complete for {}. Found {} errors", field_type, errors.len());
                            
                            let response = GrammarCheckResponse {
                                errors,
                                field_type,
                                request_id,
                            };
                            
                            if let Err(e) = response_sender.send(response) {
                                log::error!("Failed to send grammar check response: {}", e);
                            }
                        } else {
                            // Put it back if not ready yet
                            pending_check = Some((text, field_type, request_id, start_time));
                        }
                    }
                }
            }
        }
        
        log::info!("Async grammar checker background task ended");
    }
}

impl Drop for AsyncGrammarChecker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Helper to determine field type from compose field enum
pub fn compose_field_to_string(field: &crate::app::ComposeField) -> String {
    match field {
        crate::app::ComposeField::To => "To".to_string(),
        crate::app::ComposeField::Cc => "Cc".to_string(),
        crate::app::ComposeField::Bcc => "Bcc".to_string(),
        crate::app::ComposeField::Subject => "Subject".to_string(),
        crate::app::ComposeField::Body => "Body".to_string(),
    }
}
