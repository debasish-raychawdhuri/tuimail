use anyhow::{Context, Result};
use std::collections::HashSet;

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

    /// Load common English words (basic implementation)
    fn load_common_words() -> HashSet<String> {
        let mut words = HashSet::new();
        
        // Add some common English words for basic spell checking
        let common_words = vec![
            "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not", "on", "with", "he", "as", "you", "do", "at",
            "this", "but", "his", "by", "from", "they", "we", "say", "her", "she", "or", "an", "will", "my", "one", "all", "would", "there", "their",
            "what", "so", "up", "out", "if", "about", "who", "get", "which", "go", "me", "when", "make", "can", "like", "time", "no", "just", "him",
            "know", "take", "people", "into", "year", "your", "good", "some", "could", "them", "see", "other", "than", "then", "now", "look", "only",
            "come", "its", "over", "think", "also", "back", "after", "use", "two", "how", "our", "work", "first", "well", "way", "even", "new", "want",
            "because", "any", "these", "give", "day", "most", "us", "is", "was", "are", "been", "has", "had", "were", "said", "each", "which", "their",
            "time", "will", "about", "if", "up", "out", "many", "then", "them", "these", "so", "some", "her", "would", "make", "like", "into", "him",
            "two", "more", "very", "what", "know", "just", "first", "get", "over", "think", "where", "much", "go", "well", "were", "been", "through",
            "when", "who", "oil", "sit", "but", "now", "under", "last", "here", "think", "how", "too", "any", "may", "say", "she", "use", "her", "all",
            "there", "each", "which", "do", "their", "time", "if", "will", "way", "about", "out", "up", "them", "then", "she", "many", "some", "what",
            "would", "make", "like", "him", "into", "more", "two", "go", "see", "no", "could", "my", "than", "first", "been", "call", "who", "its", "now",
            "find", "long", "down", "day", "did", "get", "come", "made", "may", "part", "over", "new", "sound", "take", "only", "little", "work", "know",
            "place", "year", "live", "me", "back", "give", "most", "very", "after", "thing", "our", "just", "name", "good", "sentence", "man", "think",
            "say", "great", "where", "help", "through", "much", "before", "line", "right", "too", "mean", "old", "any", "same", "tell", "boy", "follow",
            "came", "want", "show", "also", "around", "form", "three", "small", "set", "put", "end", "why", "again", "turn", "here", "off", "went", "need",
            "should", "home", "about", "while", "sound", "below", "saw", "something", "thought", "both", "few", "those", "always", "looked", "show", "large",
            "often", "together", "asked", "house", "don't", "world", "going", "want", "school", "important", "until", "form", "food", "keep", "children",
            "feet", "land", "side", "without", "boy", "once", "animal", "life", "enough", "took", "sometimes", "four", "head", "above", "kind", "began",
            "almost", "live", "page", "got", "earth", "need", "far", "hand", "high", "year", "mother", "light", "country", "father", "let", "night", "picture",
            "being", "study", "second", "book", "carry", "took", "science", "eat", "room", "friend", "began", "idea", "fish", "mountain", "north", "once",
            "base", "hear", "horse", "cut", "sure", "watch", "color", "face", "wood", "main", "enough", "plain", "girl", "usual", "young", "ready", "above",
            "ever", "red", "list", "though", "feel", "talk", "bird", "soon", "body", "dog", "family", "direct", "pose", "leave", "song", "measure", "door",
            "product", "black", "short", "numeral", "class", "wind", "question", "happen", "complete", "ship", "area", "half", "rock", "order", "fire",
            "south", "problem", "piece", "told", "knew", "pass", "since", "top", "whole", "king", "space", "heard", "best", "hour", "better", "during",
            "hundred", "five", "remember", "step", "early", "hold", "west", "ground", "interest", "reach", "fast", "verb", "sing", "listen", "six", "table",
            "travel", "less", "morning", "ten", "simple", "several", "vowel", "toward", "war", "lay", "against", "pattern", "slow", "center", "love",
            "person", "money", "serve", "appear", "road", "map", "rain", "rule", "govern", "pull", "cold", "notice", "voice", "unit", "power", "town",
            "fine", "certain", "fly", "fall", "lead", "cry", "dark", "machine", "note", "wait", "plan", "figure", "star", "box", "noun", "field", "rest",
            "correct", "able", "pound", "done", "beauty", "drive", "stood", "contain", "front", "teach", "week", "final", "gave", "green", "oh", "quick",
            "develop", "ocean", "warm", "free", "minute", "strong", "special", "mind", "behind", "clear", "tail", "produce", "fact", "street", "inch",
            "multiply", "nothing", "course", "stay", "wheel", "full", "force", "blue", "object", "decide", "surface", "deep", "moon", "island", "foot",
            "system", "busy", "test", "record", "boat", "common", "gold", "possible", "plane", "stead", "dry", "wonder", "laugh", "thousands", "ago",
            "ran", "check", "game", "shape", "equate", "hot", "miss", "brought", "heat", "snow", "tire", "bring", "yes", "distant", "fill", "east",
            "paint", "language", "among", "grand", "ball", "yet", "wave", "drop", "heart", "am", "present", "heavy", "dance", "engine", "position",
            "arm", "wide", "sail", "material", "size", "vary", "settle", "speak", "weight", "general", "ice", "matter", "circle", "pair", "include",
            "divide", "syllable", "felt", "perhaps", "pick", "sudden", "count", "square", "reason", "length", "represent", "art", "subject", "region",
            "energy", "hunt", "probable", "bed", "brother", "egg", "ride", "cell", "believe", "fraction", "forest", "sit", "race", "window", "store",
            "summer", "train", "sleep", "prove", "lone", "leg", "exercise", "wall", "catch", "mount", "wish", "sky", "board", "joy", "winter", "sat",
            "written", "wild", "instrument", "kept", "glass", "grass", "cow", "job", "edge", "sign", "visit", "past", "soft", "fun", "bright", "gas",
            "weather", "month", "million", "bear", "finish", "happy", "hope", "flower", "clothe", "strange", "gone", "jump", "baby", "eight", "village",
            "meet", "root", "buy", "raise", "solve", "metal", "whether", "push", "seven", "paragraph", "third", "shall", "held", "hair", "describe",
            "cook", "floor", "either", "result", "burn", "hill", "safe", "cat", "century", "consider", "type", "law", "bit", "coast", "copy", "phrase",
            "silent", "tall", "sand", "soil", "roll", "temperature", "finger", "industry", "value", "fight", "lie", "beat", "excite", "natural", "view",
            "sense", "ear", "else", "quite", "broke", "case", "middle", "kill", "son", "lake", "moment", "scale", "loud", "spring", "observe", "child",
            "straight", "consonant", "nation", "dictionary", "milk", "speed", "method", "organ", "pay", "age", "section", "dress", "cloud", "surprise",
            "quiet", "stone", "tiny", "climb", "bad", "oil", "blood", "touch", "grew", "cent", "mix", "team", "wire", "cost", "lost", "brown", "wear",
            "garden", "equal", "sent", "choose", "fell", "fit", "flow", "fair", "bank", "collect", "save", "control", "decimal", "gentle", "woman",
            "captain", "practice", "separate", "difficult", "doctor", "please", "protect", "noon", "whose", "locate", "ring", "character", "insect",
            "caught", "period", "indicate", "radio", "spoke", "atom", "human", "history", "effect", "electric", "expect", "crop", "modern", "element",
            "hit", "student", "corner", "party", "supply", "bone", "rail", "imagine", "provide", "agree", "thus", "capital", "won't", "chair", "danger",
            "fruit", "rich", "thick", "soldier", "process", "operate", "guess", "necessary", "sharp", "wing", "create", "neighbor", "wash", "bat",
            "rather", "crowd", "corn", "compare", "poem", "string", "bell", "depend", "meat", "rub", "tube", "famous", "dollar", "stream", "fear",
            "sight", "thin", "triangle", "planet", "hurry", "chief", "colony", "clock", "mine", "tie", "enter", "major", "fresh", "search", "send",
            "yellow", "gun", "allow", "print", "dead", "spot", "desert", "suit", "current", "lift", "rose", "continue", "block", "chart", "hat", "sell",
            "success", "company", "subtract", "event", "particular", "deal", "swim", "term", "opposite", "wife", "shoe", "shoulder", "spread", "arrange",
            "camp", "invent", "cotton", "born", "determine", "quart", "nine", "truck", "noise", "level", "chance", "gather", "shop", "stretch", "throw",
            "shine", "property", "column", "molecule", "select", "wrong", "gray", "repeat", "require", "broad", "prepare", "salt", "nose", "plural",
            "anger", "claim", "continent", "oxygen", "sugar", "death", "pretty", "skill", "women", "season", "solution", "magnet", "silver", "thank",
            "branch", "match", "suffix", "especially", "fig", "afraid", "huge", "sister", "steel", "discuss", "forward", "similar", "guide", "experience",
            "score", "apple", "bought", "led", "pitch", "coat", "mass", "card", "band", "rope", "slip", "win", "dream", "evening", "condition", "feed",
            "tool", "total", "basic", "smell", "valley", "nor", "double", "seat", "arrive", "master", "track", "parent", "shore", "division", "sheet",
            "substance", "favor", "connect", "post", "spend", "chord", "fat", "glad", "original", "share", "station", "dad", "bread", "charge", "proper",
            "bar", "offer", "segment", "slave", "duck", "instant", "market", "degree", "populate", "chick", "dear", "enemy", "reply", "drink", "occur",
            "support", "speech", "nature", "range", "steam", "motion", "path", "liquid", "log", "meant", "quotient", "teeth", "shell", "neck"
        ];
        
        for word in common_words {
            words.insert(word.to_string());
        }
        
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

    /// Get spelling suggestions for a word (basic implementation)
    pub fn suggest(&self, word: &str) -> Vec<String> {
        if word.is_empty() {
            return Vec::new();
        }

        let mut suggestions = Vec::new();
        let word_lower = word.to_lowercase();

        // Simple suggestion algorithm: find words with similar length and starting letter
        for common_word in &self.common_words {
            if common_word.len() == word_lower.len() && 
               common_word.starts_with(&word_lower[..1]) {
                suggestions.push(common_word.clone());
                if suggestions.len() >= 5 {
                    break;
                }
            }
        }

        // If no suggestions found, try words with similar starting letters
        if suggestions.is_empty() {
            for common_word in &self.common_words {
                if common_word.starts_with(&word_lower[..1.min(word_lower.len())]) {
                    suggestions.push(common_word.clone());
                    if suggestions.len() >= 3 {
                        break;
                    }
                }
            }
        }

        suggestions
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
    fn test_common_words() {
        let config = SpellCheckConfig::default();
        let checker = SpellChecker::new(&config).unwrap();
        
        assert!(checker.is_correct("the"));
        assert!(checker.is_correct("hello"));
        assert!(checker.is_correct("world"));
        assert!(!checker.is_correct("asdfghjkl")); // nonsense word
    }
}
