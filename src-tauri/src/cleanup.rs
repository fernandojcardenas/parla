//! Light, rule-based tidying of a raw transcript. No AI model involved.

/// Words that only fill pauses. Removed when filler removal is on.
const FILLERS: &[&str] = &["um", "umm", "uh", "uhh", "er", "erm", "ah", "hmm", "mm"];

/// Text Whisper sometimes emits for non-speech sounds.
fn is_noise_tag(word: &str) -> bool {
    let w = word.trim();
    (w.starts_with('[') && w.ends_with(']')) || (w.starts_with('(') && w.ends_with(')'))
}

pub fn clean(raw: &str, remove_fillers: bool) -> String {
    let mut words: Vec<String> = Vec::new();
    for word in raw.split_whitespace() {
        if is_noise_tag(word) {
            continue;
        }
        if remove_fillers {
            let core: String = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if FILLERS.contains(&core.as_str()) {
                // Keep sentence-ending punctuation that was attached to the filler.
                if let Some(last) = words.last_mut() {
                    if word.ends_with(['.', '?', '!']) && !last.ends_with(['.', '?', '!']) {
                        last.push(word.chars().last().unwrap());
                    }
                }
                continue;
            }
        }
        words.push(word.to_string());
    }

    let mut out = words.join(" ");
    // Removing a filler can leave ", ," or a leading comma behind.
    while out.contains(", ,") {
        out = out.replace(", ,", ",");
    }
    let out = out.trim_start_matches([',', ' ']).trim().to_string();
    capitalize_first(&out)
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_fillers() {
        assert_eq!(clean(" Um, so I think, uh, we should go.", true), "So I think, we should go.");
    }

    #[test]
    fn keeps_fillers_when_off() {
        assert_eq!(clean("um hello", false), "Um hello");
    }

    #[test]
    fn drops_noise_tags() {
        assert_eq!(clean("[BLANK_AUDIO]", true), "");
        assert_eq!(clean("Hello (coughs) there", true), "Hello there");
    }

    #[test]
    fn keeps_real_words_that_contain_fillers() {
        assert_eq!(clean("Umbrella and error", true), "Umbrella and error");
    }
}
