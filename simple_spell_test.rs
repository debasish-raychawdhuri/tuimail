use std::path::Path;

// Add the src directory to the module path
mod spellcheck;

fn main() {
    println!("Testing spell checker...");
    
    let config = spellcheck::SpellCheckConfig::default();
    let checker = spellcheck::SpellChecker::new(&config).unwrap();
    
    // Test the problematic sentence
    let test_text = "That cat is.";
    println!("Testing text: '{}'", test_text);
    
    let errors = checker.check_text(test_text, &config);
    println!("Found {} spelling errors:", errors.len());
    
    for error in &errors {
        println!("  - '{}' at position {} (suggestions: {:?})", 
                 error.word, error.position, error.suggestions);
    }
    
    // Test individual words
    println!("\nTesting individual words:");
    let test_words = vec!["That", "cat", "is"];
    for word in test_words {
        let is_correct = checker.is_correct(word);
        println!("  '{}' is correct: {}", word, is_correct);
        if !is_correct {
            let suggestions = checker.suggest(word);
            println!("    Suggestions: {:?}", suggestions);
        }
    }
    
    // Test word extraction
    println!("\nWord extraction:");
    let words = spellcheck::SpellChecker::extract_words_static(test_text);
    for word_match in words {
        println!("  Word: '{}' at position {}", word_match.word, word_match.position);
    }
}
