use regex::Regex;
use std::collections::HashMap;

// Dynamic value handling
pub struct DynamicValueProcessor {
    patterns: Vec<(Regex, String)>,
    values: HashMap<String, String>,
}

impl DynamicValueProcessor {
    // Create a new dynamic value processor
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            values: HashMap::new(),
        }
    }

    // Add a pattern
    pub fn add_pattern(&mut self, pattern: &str, generator: &str) -> Result<(), String> {
        let regex = Regex::new(pattern)
            .map_err(|e| format!("Invalid regex pattern: {}", e))?;

        self.patterns.push((regex, generator.to_string()));
        Ok(())
    }

    // Process request body, extracting and replacing dynamic values
    pub fn process_request(&mut self, body: &str) -> String {
        let mut result = body.to_string();

        // Extract dynamic values
        for (regex, generator) in &self.patterns {
            let captures = regex.captures_iter(&body);

            for capture in captures {
                if let Some(matched) = capture.get(0) {
                    let full_match = matched.as_str();

                    // Check if we've seen this value before
                    if !self.values.contains_key(full_match) {
                        // Generate a new value
                        let new_value = self.generate_value(generator, &capture);
                        self.values.insert(full_match.to_string(), new_value);
                    }

                    // Replace with consistent value
                    if let Some(replacement) = self.values.get(full_match) {
                        result = result.replace(full_match, replacement);
                    }
                }
            }
        }

        result
    }

    // Generate a new value based on the generator type
    fn generate_value(&self, generator: &str, capture: &regex::Captures) -> String {
        match generator {
            "consistent_random" => {
                // Generate a random string that will be consistent for this pattern
                use rand::{thread_rng, Rng};
                use rand::distributions::Alphanumeric;

                let length = 10; // Default length

                thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(length)
                    .map(char::from)
                    .collect()
            },
            "increment" => {
                // Increment a value
                if let Some(group) = capture.get(1) {
                    if let Ok(num) = group.as_str().parse::<i64>() {
                        return (num + 1).to_string();
                    }
                }
                "1".to_string()
            },
            // Add more generators as needed
            _ => generator.to_string(),
        }
    }
}
