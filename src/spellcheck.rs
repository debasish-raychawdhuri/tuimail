use anyhow::{Context, Result};
use std::collections::HashSet;

// Embed the practical word lists at compile time
const COMMON_WORDS: &str = include_str!("../resources/google-10000-english.txt");
const TECHNICAL_TERMS: &str = include_str!("../resources/technical-terms.txt");
const ADDITIONAL_COMMON: &str = include_str!("../resources/additional-common.txt");

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

    /// Load practical English dictionary (10k most common + technical terms + additional common words)
    fn load_common_words() -> HashSet<String> {
        let mut words = HashSet::new();
        
        // Load Google's 10,000 most common English words
        for line in COMMON_WORDS.lines() {
            let word = line.trim();
            if !word.is_empty() && word.len() >= 2 {
                // Convert to lowercase for case-insensitive matching
                words.insert(word.to_lowercase());
                
                // Also add capitalized version for sentence starts
                let capitalized = format!("{}{}", 
                    word.chars().next().unwrap().to_uppercase(),
                    &word[1..]
                );
                words.insert(capitalized);
            }
        }
        
        // Load technical and business terms relevant to email
        for line in TECHNICAL_TERMS.lines() {
            let word = line.trim();
            if !word.is_empty() && word.len() >= 2 {
                words.insert(word.to_lowercase());
                
                // Add capitalized version
                let capitalized = format!("{}{}", 
                    word.chars().next().unwrap().to_uppercase(),
                    &word[1..]
                );
                words.insert(capitalized);
            }
        }
        
        // Load additional common words that might not be in top 10k
        for line in ADDITIONAL_COMMON.lines() {
            let word = line.trim();
            if !word.is_empty() && word.len() >= 2 {
                words.insert(word.to_lowercase());
                
                // Add capitalized version
                let capitalized = format!("{}{}", 
                    word.chars().next().unwrap().to_uppercase(),
                    &word[1..]
                );
                words.insert(capitalized);
            }
        }
        
        // Add common contractions that might not be in the lists
        let contractions = vec![
            "don't", "won't", "can't", "shouldn't", "wouldn't", "couldn't", "didn't",
            "isn't", "aren't", "wasn't", "weren't", "haven't", "hasn't", "hadn't",
            "I'm", "you're", "he's", "she's", "it's", "we're", "they're",
            "I've", "you've", "we've", "they've", "I'll", "you'll", "he'll",
            "she'll", "it'll", "we'll", "they'll", "I'd", "you'd", "he'd",
            "she'd", "we'd", "they'd", "let's", "that's", "what's", "where's",
            "when's", "why's", "how's", "here's", "there's"
        ];
        
        for contraction in contractions {
            words.insert(contraction.to_string());
        }
        
        log::info!("Loaded {} words into practical spell checker dictionary", words.len());
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

        // Strategy 1: Find exact matches with different case
        for common_word in &self.common_words {
            if common_word.to_lowercase() == word_lower && common_word != &word {
                suggestions.push(common_word.clone());
            }
        }

        // Strategy 2: Find words with high similarity (edit distance 1-2)
        for common_word in &self.common_words {
            if (common_word.len() as i32 - word_len as i32).abs() <= 2 {
                let similarity = self.calculate_similarity(&word_lower, common_word);
                if similarity >= 0.6 {
                    if !suggestions.contains(common_word) {
                        suggestions.push(common_word.clone());
                    }
                }
            }
        }

        // Strategy 3: Common misspelling patterns
        let corrected = self.apply_common_corrections(&word_lower);
        if corrected != word_lower && self.common_words.contains(&corrected) {
            if !suggestions.contains(&corrected) {
                suggestions.insert(0, corrected); // Put common corrections first
            }
        }

        // Strategy 4: If still not enough, try prefix matching for longer words
        if suggestions.len() < 3 && word_len >= 4 {
            let prefix = &word_lower[..3];
            for common_word in &self.common_words {
                if common_word.starts_with(prefix) && !suggestions.contains(common_word) {
                    suggestions.push(common_word.clone());
                    if suggestions.len() >= 5 {
                        break;
                    }
                }
            }
        }

        // Sort by similarity and length
        suggestions.sort_by(|a, b| {
            let a_sim = self.calculate_similarity(&word_lower, a);
            let b_sim = self.calculate_similarity(&word_lower, b);
            b_sim.partial_cmp(&a_sim).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        suggestions.truncate(5); // Limit to 5 suggestions
        suggestions
    }

    /// Apply common spelling correction patterns
    fn apply_common_corrections(&self, word: &str) -> String {
        let mut corrected = word.to_string();
        
        // Common misspelling patterns
        let patterns = vec![
            ("teh", "the"),
            ("recieve", "receive"),
            ("seperate", "separate"),
            ("occured", "occurred"),
            ("neccessary", "necessary"),
            ("definately", "definitely"),
            ("accomodate", "accommodate"),
            ("begining", "beginning"),
            ("beleive", "believe"),
            ("calender", "calendar"),
            ("cemetary", "cemetery"),
            ("changable", "changeable"),
            ("collegue", "colleague"),
            ("comming", "coming"),
            ("concious", "conscious"),
            ("dilemna", "dilemma"),
            ("embarass", "embarrass"),
            ("enviroment", "environment"),
            ("existance", "existence"),
            ("goverment", "government"),
            ("harrass", "harass"),
            ("independant", "independent"),
            ("judgement", "judgment"),
            ("knowlege", "knowledge"),
            ("liason", "liaison"),
            ("maintainance", "maintenance"),
            ("noticable", "noticeable"),
            ("occassion", "occasion"),
            ("perseverence", "perseverance"),
            ("priviledge", "privilege"),
            ("questionaire", "questionnaire"),
            ("recomend", "recommend"),
            ("succesful", "successful"),
            ("tommorow", "tomorrow"),
            ("untill", "until"),
            ("vaccuum", "vacuum"),
            ("wierd", "weird"),
        ];
        
        for (wrong, right) in patterns {
            if word == wrong {
                return right.to_string();
            }
        }
        
        // Single character corrections
        if word.ends_with("tion") && word.len() > 4 {
            // Check for -sion vs -tion
            let sion_version = format!("{}sion", &word[..word.len()-4]);
            if self.common_words.contains(&sion_version) {
                return sion_version;
            }
        }
        
        corrected
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
        
        log::debug!("Spell checking text: '{}'", text);
        let words = Self::extract_words(text);
        log::debug!("Extracted {} words: {:?}", words.len(), words.iter().map(|w| &w.word).collect::<Vec<_>>());

        for word_match in words {
            let word = word_match.word;
            let word_pos = word_match.position;

            // Skip words based on configuration
            if self.should_skip_word(&word, config) {
                log::debug!("Skipping word: '{}'", word);
                continue;
            }

            let is_correct = self.is_correct(&word);
            log::debug!("Word '{}' is correct: {}", word, is_correct);
            
            if !is_correct {
                let suggestions = self.suggest(&word);
                let limited_suggestions = suggestions
                    .into_iter()
                    .take(config.max_suggestions)
                    .collect();

                log::debug!("Found spelling error: '{}' at position {}, suggestions: {:?}", word, word_pos, limited_suggestions);
                errors.push(SpellError {
                    word: word.to_string(),
                    position: word_pos,
                    suggestions: limited_suggestions,
                });
            }
        }

        log::debug!("Total spelling errors found: {}", errors.len());
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
    fn test_original_case() {
        let config = SpellCheckConfig::default();
        let checker = SpellChecker::new(&config).unwrap();
        
        // Test the original problematic text
        let test_text = "Ther was a brown crow; have you ever seen a brown crow?";
        let errors = checker.check_text(test_text, &config);
        
        println!("Testing text: '{}'", test_text);
        println!("Found {} spelling errors:", errors.len());
        
        for error in &errors {
            println!("  - '{}' at position {} (suggestions: {:?})", 
                     error.word, error.position, error.suggestions);
        }
        
        // Should find "Ther" as an error now
        assert!(errors.len() > 0, "Should find at least one spelling error");
        
        let ther_error = errors.iter().find(|e| e.word == "Ther");
        assert!(ther_error.is_some(), "Should find 'Ther' as a spelling error");
        
        // Verify individual words
        assert!(!checker.is_correct("Ther"), "'Ther' should be incorrect");
        assert!(checker.is_correct("There"), "'There' should be correct");
        assert!(checker.is_correct("was"), "'was' should be correct");
        assert!(checker.is_correct("brown"), "'brown' should be correct");
        assert!(checker.is_correct("crow"), "'crow' should be correct");
    }
}
