// Test specific grammar cases
use std::env;

// Include the modules we need
mod grammarcheck;

fn main() {
    // Set up logging to see debug output
    env::set_var("RUST_LOG", "debug");
    env_logger::init();
    
    println!("Testing specific grammar cases...");
    
    let checker = match grammarcheck::GrammarChecker::new() {
        Ok(checker) => checker,
        Err(e) => {
            println!("Failed to create grammar checker: {}", e);
            return;
        }
    };
    
    let config = grammarcheck::GrammarCheckConfig::default();
    
    let test_cases = vec![
        "That cat is.",                    // Incomplete sentence - should be flagged
        "That cat is sleeping.",           // Complete sentence - should be OK
        "She was not been here.",          // Grammar error - should be flagged
        "She was not here.",               // Correct - should be OK
        "He don't like it.",               // Grammar error - should be flagged
        "He doesn't like it.",             // Correct - should be OK
        "I are going home.",               // Subject-verb disagreement - should be flagged
        "I am going home.",                // Correct - should be OK
    ];
    
    for (i, test_text) in test_cases.iter().enumerate() {
        println!("\n--- Test Case {} ---", i + 1);
        println!("Text: '{}'", test_text);
        
        let errors = checker.check_text(test_text, &config);
        println!("Grammar errors found: {}", errors.len());
        
        for (j, error) in errors.iter().enumerate() {
            println!("  Error {}: '{}' at positions {}-{}", 
                     j + 1, 
                     error.message, 
                     error.start, 
                     error.end);
            println!("    Replacements: {:?}", error.replacements);
            println!("    Source: '{}'", error.source);
        }
        
        if errors.is_empty() {
            println!("  No grammar errors detected.");
        }
        
        // Also test the correction function
        let corrected = checker.correct_text(test_text);
        if corrected != *test_text {
            println!("  Corrected text: '{}'", corrected);
        } else {
            println!("  No corrections applied.");
        }
    }
}
