// Test the grammar checker directly
use std::process::Command;

fn main() {
    println!("Testing grammar checker with nlprule...");
    
    // Test if nlprule can detect the grammar error
    let test_sentences = vec![
        "That cat is.",           // Incomplete sentence
        "That cat is sleeping.",  // Complete sentence
        "She was not been here.", // Grammar error
        "She was not here.",      // Correct
        "He don't like it.",      // Grammar error
        "He doesn't like it.",    // Correct
    ];
    
    for sentence in test_sentences {
        println!("\nTesting: '{}'", sentence);
        
        // Use a simple Python script to test nlprule
        let python_code = format!(r#"
import nlprule

# Load English tokenizer and rules
tokenizer = nlprule.Tokenizer.load("en")
rules = nlprule.Rules.load("en", tokenizer)

# Check the sentence
text = "{}"
suggestions = rules.suggest(text)

print(f"Text: {{text}}")
print(f"Suggestions: {{len(suggestions)}}")
for i, suggestion in enumerate(suggestions):
    print(f"  {{i+1}}. {{suggestion.message}} ({{suggestion.start}}-{{suggestion.end}})")
    print(f"      Replacements: {{suggestion.replacements}}")
"#, sentence);
        
        // Write Python code to a temporary file
        std::fs::write("/tmp/test_grammar.py", python_code).unwrap();
        
        // Run the Python script
        let output = Command::new("python3")
            .arg("/tmp/test_grammar.py")
            .output();
            
        match output {
            Ok(result) => {
                if result.status.success() {
                    println!("{}", String::from_utf8_lossy(&result.stdout));
                } else {
                    println!("Error: {}", String::from_utf8_lossy(&result.stderr));
                }
            }
            Err(e) => {
                println!("Failed to run Python: {}", e);
                break;
            }
        }
    }
}
