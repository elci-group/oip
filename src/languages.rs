// -----------------------------
// src/languages.rs
// -----------------------------

pub enum Language {
    Rust,
    Python,
    Javascript,
    Typescript,
    Other(String),
}

impl Language {
    /// Detect language from text snippet
    pub fn from_text(text: &str) -> Self {
        let lower = text.to_lowercase();
        if lower.contains("fn") && lower.contains("let") {
            Language::Rust
        } else if lower.contains("def") && lower.contains(":") {
            Language::Python
        } else if lower.contains("function") && lower.contains("{") {
            if lower.contains("typescript") {
                Language::Typescript
            } else {
                Language::Javascript
            }
        } else {
            Language::Other("txt".to_string())
        }
    }
}
