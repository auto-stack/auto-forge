//! Relay Run Title Generator
//!
//! Auto-generates concise display titles from relay task descriptions.
//! Algorithm based on specs/designs.ad — Auto-Generated Relay Run Titles.

const MAX_TITLE_LEN: usize = 40;
const DEFAULT_TITLE: &str = "Untitled Relay";

/// Generate a concise title from a relay task description.
pub fn generate_title(task: &str) -> String {
    let task = task.trim();

    if task.is_empty() {
        return DEFAULT_TITLE.to_string();
    }

    // Step 1: Remove leading action verbs
    let task = strip_action_verbs(task);

    // Step 2: Remove leading filler words
    let task = strip_filler_words(task);

    // Step 3: Extract first phrase (up to 6 words, prefer first 3)
    let words: Vec<&str> = task.split_whitespace().collect();
    let phrase = if words.len() >= 3 {
        words[..=2].join(" ")
    } else {
        words.join(" ")
    };

    // Step 4: Truncate if too long
    let phrase = if phrase.len() > MAX_TITLE_LEN {
        format!("{}...", &phrase[..MAX_TITLE_LEN.saturating_sub(3)])
    } else {
        phrase
    };

    // Step 5: Capitalize each word
    capitalize_words(&phrase)
}

fn strip_action_verbs(task: &str) -> &str {
    let action_verbs = [
        "build", "create", "implement", "add", "fix", "update",
        "write", "generate", "design", "develop", "refactor",
        "make", "set", "get", "delete", "remove",
    ];

    let lower = task.to_lowercase();
    for verb in &action_verbs {
        if lower.starts_with(verb) {
            let rest = &task[verb.len()..];
            return rest.trim_start_matches(|c: char| c == ' ' || c == ':' || c == '-');
        }
    }

    task
}

fn strip_filler_words(task: &str) -> &str {
    let fillers = ["a", "an", "the", "with", "for", "from"];

    let lower = task.to_lowercase();
    for filler in &fillers {
        if lower.starts_with(filler) {
            let rest = &task[filler.len()..];
            // Only strip if it's a complete word (followed by space or end of string)
            if rest.is_empty() || rest.starts_with(' ') {
                return rest.trim_start_matches(|c: char| c == ' ');
            }
        }
    }

    task
}

fn capitalize_words(phrase: &str) -> String {
    phrase
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_extraction() {
        assert_eq!(
            generate_title("Build a simple cache module"),
            "Simple Cache Module"
        );
    }

    #[test]
    fn test_truncation() {
        let long = "Supercalifragilisticexpialidocious antidisestablishmentarianism pneumonoultramicroscopicsilicovolcanoconiosis mechanism";
        let result = generate_title(long);
        assert!(result.len() <= MAX_TITLE_LEN + 3); // +3 for "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_empty_task() {
        assert_eq!(generate_title(""), "Untitled Relay");
        assert_eq!(generate_title("   "), "Untitled Relay");
    }

    #[test]
    fn test_single_word() {
        assert_eq!(generate_title("cache"), "Cache");
    }

    #[test]
    fn test_action_verb_stripping() {
        assert_eq!(
            generate_title("Implement caching layer for API responses"),
            "Caching Layer For"
        );
        assert_eq!(
            generate_title("Fix authentication bug in login flow"),
            "Authentication Bug In"
        );
    }

    #[test]
    fn test_filler_word_stripping() {
        assert_eq!(
            generate_title("a simple cache module"),
            "Simple Cache Module"
        );
    }

    #[test]
    fn test_relay_run_title() {
        assert_eq!(
            generate_title("Implement relay run title feature: auto-generate short titles from task descriptions"),
            "Relay Run Title"
        );
    }
}
