use std::collections::HashSet;

// Simple test to verify spell checking works
fn main() {
    // Test the spell checker directly
    let config = tuimail::spellcheck::SpellCheckConfig::default();
    let checker = tuimail::spellcheck::SpellChecker::new(&config).unwrap();
    
    let test_text = "Ther was a brown crow; have you ever seen a brown crow?";
    println!("Testing text: '{}'", test_text);
    
    let errors = checker.check_text(test_text, &config);
    println!("Found {} spelling errors:", errors.len());
    
    for error in &errors {
        println!("  - '{}' at position {} (suggestions: {:?})", 
                 error.word, error.position, error.suggestions);
    }
    
    // Test individual words
    println!("\nTesting individual words:");
    let test_words = vec!["Ther", "There", "was", "brown", "crow", "asdfgh"];
    for word in test_words {
        let is_correct = checker.is_correct(word);
        println!("  '{}' is correct: {}", word, is_correct);
        if !is_correct {
            let suggestions = checker.suggest(word);
            println!("    Suggestions: {:?}", suggestions);
        }
    }
}
