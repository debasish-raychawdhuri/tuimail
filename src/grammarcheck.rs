use anyhow::{Context, Result};
use nlprule::{Rules, Tokenizer, rules::apply_suggestions, types::Suggestion};
use std::sync::Arc;

/// Grammar checker for email composition
pub struct GrammarChecker {
    tokenizer: Arc<Tokenizer>,
    rules: Arc<Rules>,
}

/// Represents a grammar error with suggestions
#[derive(Debug, Clone)]
pub struct GrammarError {
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub replacements: Vec<String>,
    pub source: String,
}

/// Configuration for grammar checking
#[derive(Debug, Clone)]
pub struct GrammarCheckConfig {
    pub enabled: bool,
    pub language: String,
    pub max_suggestions: usize,
}

impl Default for GrammarCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            language: "en".to_string(),
            max_suggestions: 5,
        }
    }
}

impl GrammarChecker {
    /// Create a new grammar checker
    pub fn new() -> Result<Self> {
        // Include the binary resources at compile time
        let mut tokenizer_bytes: &[u8] = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/",
            nlprule::tokenizer_filename!("en")
        ));
        let mut rules_bytes: &[u8] = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/",
            nlprule::rules_filename!("en")
        ));

        // Load the tokenizer and rules
        let tokenizer = Tokenizer::from_reader(&mut tokenizer_bytes)
            .context("Failed to load tokenizer binary")?;
        let rules = Rules::from_reader(&mut rules_bytes)
            .context("Failed to load rules binary")?;

        Ok(Self {
            tokenizer: Arc::new(tokenizer),
            rules: Arc::new(rules),
        })
    }

    /// Check grammar in text and return errors
    pub fn check_text(&self, text: &str, config: &GrammarCheckConfig) -> Vec<GrammarError> {
        if !config.enabled || text.trim().is_empty() {
            return Vec::new();
        }

        log::debug!("Grammar checking text: '{}'", text);
        
        // Get suggestions from nlprule
        let suggestions = self.rules.suggest(text, &self.tokenizer);
        
        log::debug!("nlprule returned {} suggestions", suggestions.len());
        for (i, suggestion) in suggestions.iter().enumerate() {
            let span = suggestion.span().char();
            log::debug!("  Suggestion {}: '{}' at pos {}-{}, replacements: {:?}", 
                i + 1, 
                suggestion.message(), 
                span.start, 
                span.end,
                suggestion.replacements()
            );
        }
        
        // Convert to our GrammarError format
        let errors: Vec<GrammarError> = suggestions
            .iter()
            .map(|s| {
                let span = s.span().char();
                GrammarError {
                    message: s.message().to_string(),
                    start: span.start,
                    end: span.end,
                    replacements: s.replacements().iter().cloned().collect(),
                    source: s.source().to_string(),
                }
            })
            .collect();

        log::debug!("Total grammar errors found: {}", errors.len());
        errors
    }

    /// Correct grammar in text
    pub fn correct_text(&self, text: &str) -> String {
        self.rules.correct(text, &self.tokenizer)
    }

    /// Get statistics about the grammar check
    pub fn get_stats(&self, text: &str, config: &GrammarCheckConfig) -> GrammarCheckStats {
        let errors = self.check_text(text, config);
        let error_count = errors.len();
        
        // Estimate sentence count (rough approximation)
        let sentence_count = text.split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .count();
        
        GrammarCheckStats {
            sentence_count,
            error_count,
            quality_score: if sentence_count > 0 {
                100.0 - ((error_count as f64 / sentence_count as f64) * 100.0).min(100.0)
            } else {
                100.0
            },
        }
    }

    /// Apply specific grammar correction
    pub fn apply_correction(&self, text: &str, error: &GrammarError, replacement_idx: usize) -> String {
        if replacement_idx >= error.replacements.len() {
            return text.to_string();
        }

        let replacement = &error.replacements[replacement_idx];
        let before = &text[..error.start];
        let after = &text[error.end..];
        
        format!("{}{}{}", before, replacement, after)
    }
}

/// Statistics about grammar checking results
#[derive(Debug, Clone)]
pub struct GrammarCheckStats {
    pub sentence_count: usize,
    pub error_count: usize,
    pub quality_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nlprule_detection() {
        println!("Testing nlprule grammar detection...");
        
        let checker = GrammarChecker::new().unwrap();
        let config = GrammarCheckConfig::default();
        
        // Test cases with obvious grammar errors
        let test_cases = vec![
            ("That cat is.", "incomplete sentence"),
            ("Me are going", "subject-verb disagreement"),
            ("I are happy", "subject-verb disagreement"), 
            ("She don't like it", "subject-verb disagreement"),
            ("He have a car", "subject-verb disagreement"),
            ("This is a good sentence.", "correct sentence"),
            ("The cats is sleeping", "subject-verb disagreement"),
            ("I has a book", "subject-verb disagreement"),
            ("We was there yesterday", "subject-verb disagreement"),
        ];
        
        for (test_text, expected_error_type) in test_cases {
            println!("\n--- Testing: \"{}\" (expecting: {}) ---", test_text, expected_error_type);
            
            // Check for grammar errors
            let errors = checker.check_text(test_text, &config);
            
            if errors.is_empty() {
                println!("✅ No grammar errors detected");
            } else {
                println!("❌ Grammar errors found ({} errors):", errors.len());
                for (i, error) in errors.iter().enumerate() {
                    println!("  {}. Error at position {}-{}: \"{}\"", 
                        i + 1,
                        error.start,
                        error.end,
                        error.message
                    );
                    if !error.replacements.is_empty() {
                        println!("     Suggestions: [{}]", error.replacements.join(", "));
                    }
                }
            }
        }
    }

    #[test]
    fn test_grammar_checker() {
        let checker = GrammarChecker::new().unwrap();
        let config = GrammarCheckConfig::default();
        
        // Test text with known grammar errors
        let test_text = "She was not been here since Monday.";
        let errors = checker.check_text(test_text, &config);
        
        assert!(!errors.is_empty(), "Should have found grammar errors");
        
        // Test correction
        let corrected = checker.correct_text(test_text);
        assert_ne!(corrected, test_text, "Text should have been corrected");
        assert_eq!(corrected, "She was not here since Monday.");
    }

    #[test]
    fn test_stats_consistency() {
        let checker = GrammarChecker::new().unwrap();
        let config = GrammarCheckConfig::default();
        
        // Test text with known errors
        let test_text = "She was not been here since Monday. He don't like it.";
        let errors = checker.check_text(test_text, &config);
        let stats = checker.get_stats(test_text, &config);
        
        // The error count and stats should be consistent
        assert_eq!(errors.len(), stats.error_count, 
                   "Error count should match stats error_count");
        
        // For this text with errors, should have some errors
        assert!(errors.len() > 0, "Should have errors for incorrect text");
        assert!(stats.error_count > 0, "Stats should show errors");
        assert!(stats.quality_score < 100.0, "Quality score should be less than 100% for text with errors");
    }
}
