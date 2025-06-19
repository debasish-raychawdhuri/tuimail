use nlprule::{Rules, Tokenizer};

fn main() {
    println!("Testing nlprule grammar detection...");
    
    // Initialize nlprule
    let tokenizer = match Tokenizer::new("en") {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to create tokenizer: {}", e);
            return;
        }
    };
    
    let rules = match Rules::new("en") {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to create rules: {}", e);
            return;
        }
    };
    
    // Test cases
    let test_cases = vec![
        "That cat is.",
        "Me are going",
        "I are happy",
        "She don't like it",
        "He have a car",
        "This is a good sentence.",
        "The cats is sleeping",
        "I has a book",
    ];
    
    for test_text in test_cases {
        println!("\n--- Testing: \"{}\" ---", test_text);
        
        // Check for grammar errors
        let suggestions = rules.suggest(test_text, &tokenizer);
        
        if suggestions.is_empty() {
            println!("✅ No grammar errors detected");
        } else {
            println!("❌ Grammar errors found:");
            for suggestion in suggestions {
                println!("  - Error: {} -> {}", 
                    suggestion.source(), 
                    suggestion.replacements().join(", ")
                );
                println!("    Message: {}", suggestion.message());
            }
        }
    }
}
