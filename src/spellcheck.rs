use anyhow::{Context, Result};
use std::collections::HashSet;

// Embed the comprehensive English dictionary at compile time
const ENGLISH_WORDS: &str = include_str!("../resources/words.txt");

/// Spell checker for email composition
/// This is a basic implementation that can be extended with proper dictionary support
pub struct SpellChecker {
    personal_dictionary: HashSet<String>,
    common_words: HashSet<String>,
}

/// Represents a misspelled word with suggestions
#[derive(Debug, Clone)]
pub struct SpellError {
    pub word: String,
    pub position: usize,
    pub suggestions: Vec<String>,
}

/// Configuration for spell checking
#[derive(Debug, Clone)]
pub struct SpellCheckConfig {
    pub enabled: bool,
    pub language: String,
    pub max_suggestions: usize,
    pub ignore_uppercase: bool,
    pub ignore_numbers: bool,
    pub personal_dictionary_path: Option<String>,
}

impl Default for SpellCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            language: "en_US".to_string(),
            max_suggestions: 5,
            ignore_uppercase: true,
            ignore_numbers: true,
            personal_dictionary_path: None,
        }
    }
}

impl SpellChecker {
    /// Create a new spell checker with the given configuration
    pub fn new(config: &SpellCheckConfig) -> Result<Self> {
        let personal_dictionary = if let Some(path) = &config.personal_dictionary_path {
            Self::load_personal_dictionary(path)?
        } else {
            HashSet::new()
        };

        let common_words = Self::load_common_words();

        Ok(Self {
            personal_dictionary,
            common_words,
        })
    }

    /// Load comprehensive English dictionary (466,550+ words)
    fn load_common_words() -> HashSet<String> {
        let mut words = HashSet::new();
        
        // Load words from embedded dictionary
        for line in ENGLISH_WORDS.lines() {
            let word = line.trim();
            if !word.is_empty() && word.len() >= 2 {
                // Convert to lowercase for case-insensitive matching
                words.insert(word.to_lowercase());
                
                // Also add the original case for proper nouns and acronyms
                if word != word.to_lowercase() {
                    words.insert(word.to_string());
                }
            }
        }
        
        log::info!("Loaded {} words into spell checker dictionary", words.len());
        words
    }

    /// Load personal dictionary from file
    fn load_personal_dictionary(path: &str) -> Result<HashSet<String>> {
        let mut words = HashSet::new();
        
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let word = line.trim();
                if !word.is_empty() {
                    words.insert(word.to_lowercase());
                }
            }
        }
        
        Ok(words)
    }

    /// Check if a word is spelled correctly
    pub fn is_correct(&self, word: &str) -> bool {
        // Skip empty words
        if word.is_empty() {
            return true;
        }

        let word_lower = word.to_lowercase();

        // Check personal dictionary first
        if self.personal_dictionary.contains(&word_lower) {
            return true;
        }

        // Check common words
        self.common_words.contains(&word_lower)
    }

    /// Get spelling suggestions for a word (improved algorithm for large dictionary)
    pub fn suggest(&self, word: &str) -> Vec<String> {
        if word.is_empty() {
            return Vec::new();
        }

        let mut suggestions = Vec::new();
        let word_lower = word.to_lowercase();
        let word_len = word_lower.len();

        // Strategy 1: Find words with same length and similar starting characters
        for common_word in &self.common_words {
            if common_word.len() == word_len {
                let similarity = self.calculate_similarity(&word_lower, common_word);
                if similarity >= 0.7 { // 70% similarity threshold
                    suggestions.push(common_word.clone());
                    if suggestions.len() >= 3 {
                        break;
                    }
                }
            }
        }

        // Strategy 2: If not enough suggestions, try words with similar length (±1)
        if suggestions.len() < 3 {
            for common_word in &self.common_words {
                if (common_word.len() as i32 - word_len as i32).abs() <= 1 {
                    let similarity = self.calculate_similarity(&word_lower, common_word);
                    if similarity >= 0.6 { // Lower threshold for different lengths
                        suggestions.push(common_word.clone());
                        if suggestions.len() >= 5 {
                            break;
                        }
                    }
                }
            }
        }

        // Strategy 3: If still not enough, try prefix matching
        if suggestions.len() < 3 && word_len >= 3 {
            let prefix = &word_lower[..3.min(word_len)];
            for common_word in &self.common_words {
                if common_word.starts_with(prefix) && !suggestions.contains(common_word) {
                    suggestions.push(common_word.clone());
                    if suggestions.len() >= 5 {
                        break;
                    }
                }
            }
        }

        // Remove duplicates and sort by length similarity
        suggestions.sort_by(|a, b| {
            let a_diff = (a.len() as i32 - word_len as i32).abs();
            let b_diff = (b.len() as i32 - word_len as i32).abs();
            a_diff.cmp(&b_diff)
        });
        
        suggestions.truncate(5); // Limit to 5 suggestions
        suggestions
    }

    /// Calculate similarity between two words (simple Levenshtein-like algorithm)
    fn calculate_similarity(&self, word1: &str, word2: &str) -> f64 {
        if word1 == word2 {
            return 1.0;
        }
        
        let len1 = word1.len();
        let len2 = word2.len();
        
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        // Simple character-based similarity
        let chars1: Vec<char> = word1.chars().collect();
        let chars2: Vec<char> = word2.chars().collect();
        
        let mut matches = 0;
        let min_len = len1.min(len2);
        
        for i in 0..min_len {
            if chars1[i] == chars2[i] {
                matches += 1;
            }
        }
        
        // Bonus for same starting characters
        let start_bonus = if chars1[0] == chars2[0] { 0.1 } else { 0.0 };
        
        (matches as f64 / len1.max(len2) as f64) + start_bonus
    }

    /// Check spelling of entire text and return errors
    pub fn check_text(&self, text: &str, config: &SpellCheckConfig) -> Vec<SpellError> {
        let mut errors = Vec::new();

        for word_match in Self::extract_words(text) {
            let word = word_match.word;
            let word_pos = word_match.position;

            // Skip words based on configuration
            if self.should_skip_word(&word, config) {
                continue;
            }

            if !self.is_correct(&word) {
                let suggestions = self.suggest(&word);
                let limited_suggestions = suggestions
                    .into_iter()
                    .take(config.max_suggestions)
                    .collect();

                errors.push(SpellError {
                    word: word.to_string(),
                    position: word_pos,
                    suggestions: limited_suggestions,
                });
            }
        }

        errors
    }

    /// Extract words from text with their positions
    fn extract_words(text: &str) -> Vec<WordMatch> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let mut word_start = 0;
        let mut in_word = false;

        for (i, ch) in text.char_indices() {
            if ch.is_alphabetic() || ch == '\'' || ch == '-' {
                if !in_word {
                    word_start = i;
                    in_word = true;
                    current_word.clear();
                }
                current_word.push(ch);
            } else {
                if in_word {
                    words.push(WordMatch {
                        word: current_word.clone(),
                        position: word_start,
                    });
                    in_word = false;
                }
            }
        }

        // Handle word at end of text
        if in_word {
            words.push(WordMatch {
                word: current_word,
                position: word_start,
            });
        }

        words
    }

    /// Check if a word should be skipped based on configuration
    fn should_skip_word(&self, word: &str, config: &SpellCheckConfig) -> bool {
        // Skip very short words
        if word.len() < 2 {
            return true;
        }

        // Skip if configured to ignore uppercase words
        if config.ignore_uppercase && word.chars().all(|c| c.is_uppercase()) {
            return true;
        }

        // Skip if configured to ignore words with numbers
        if config.ignore_numbers && word.chars().any(|c| c.is_numeric()) {
            return true;
        }

        // Skip common email patterns
        if word.contains('@') || word.starts_with("http") || word.starts_with("www.") {
            return true;
        }

        false
    }

    /// Add word to personal dictionary
    pub fn add_to_personal_dictionary(&mut self, word: &str) {
        self.personal_dictionary.insert(word.to_lowercase());
    }

    /// Save personal dictionary to file
    pub fn save_personal_dictionary(&self, path: &str) -> Result<()> {
        let words: Vec<String> = self.personal_dictionary.iter().cloned().collect();
        let mut sorted_words = words;
        sorted_words.sort();
        
        let content = sorted_words.join("\n");
        std::fs::write(path, content)
            .context("Failed to save personal dictionary")
    }

    /// Get statistics about the spell check
    pub fn get_stats(&self, text: &str, config: &SpellCheckConfig) -> SpellCheckStats {
        let words = Self::extract_words(text);
        let total_words = words.len();
        let errors = self.check_text(text, config);
        let misspelled_words = errors.len();
        
        SpellCheckStats {
            total_words,
            misspelled_words,
            accuracy: if total_words > 0 {
                ((total_words - misspelled_words) as f64 / total_words as f64) * 100.0
            } else {
                100.0
            },
        }
    }
}

/// Word match with position information
#[derive(Debug, Clone)]
struct WordMatch {
    word: String,
    position: usize,
}

/// Statistics about spell checking results
#[derive(Debug, Clone)]
pub struct SpellCheckStats {
    pub total_words: usize,
    pub misspelled_words: usize,
    pub accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_extraction() {
        let text = "Hello world! This is a test.";
        let words = SpellChecker::extract_words(text);
        
        assert_eq!(words.len(), 6);
        assert_eq!(words[0].word, "Hello");
        assert_eq!(words[1].word, "world");
        assert_eq!(words[2].word, "This");
    }

    #[test]
    fn test_should_skip_word() {
        let config = SpellCheckConfig::default();
        let checker = SpellChecker::new(&config).unwrap();
        
        assert!(checker.should_skip_word("HTTP", &config)); // uppercase
        assert!(checker.should_skip_word("test123", &config)); // contains numbers
        assert!(checker.should_skip_word("user@example.com", &config)); // email
        assert!(!checker.should_skip_word("hello", &config)); // normal word
    }

    #[test]
    fn test_comprehensive_dictionary() {
        let config = SpellCheckConfig::default();
        let checker = SpellChecker::new(&config).unwrap();
        
        // Test common words
        assert!(checker.is_correct("the"));
        assert!(checker.is_correct("hello"));
        assert!(checker.is_correct("world"));
        assert!(checker.is_correct("computer"));
        assert!(checker.is_correct("programming"));
        assert!(checker.is_correct("email"));
        assert!(checker.is_correct("message"));
        
        // Test contractions
        assert!(checker.is_correct("don't"));
        assert!(checker.is_correct("won't"));
        assert!(checker.is_correct("can't"));
        
        // Test technical terms that should be in the dictionary
        assert!(checker.is_correct("algorithm"));
        assert!(checker.is_correct("database"));
        
        // Test nonsense words
        assert!(!checker.is_correct("asdfghjkl"));
        assert!(!checker.is_correct("qwertyuiop"));
        assert!(!checker.is_correct("zxcvbnm"));
    }

    #[test]
    fn test_suggestions_quality() {
        let config = SpellCheckConfig::default();
        let checker = SpellChecker::new(&config).unwrap();
        
        // Test suggestions for common misspellings
        let suggestions = checker.suggest("teh");
        assert!(suggestions.len() > 0);
        // The suggestion algorithm should find similar words
        
        let suggestions = checker.suggest("recieve");
        assert!(suggestions.len() > 0);
        
        let suggestions = checker.suggest("seperate");
        assert!(suggestions.len() > 0);
        
        // Test that suggestions are reasonable length
        let suggestions = checker.suggest("hello");
        assert!(suggestions.len() <= 5); // Should limit to 5 suggestions
    }
}
