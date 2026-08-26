// Copyright (c) 2026 elci-group
// SPDX-License-Identifier: MIT
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

    pub fn name(&self) -> &str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::Javascript => "JavaScript",
            Language::Typescript => "TypeScript",
            Language::Other(ext) => ext,
        }
    }

    /// File extensions (without the leading dot) that a candidate target
    /// file for this language may use, used by `infer_target_file` to
    /// search the project for the file the clipboard content belongs to.
    pub fn extensions(&self) -> &[&str] {
        match self {
            Language::Rust => &["rs"],
            Language::Python => &["py"],
            Language::Javascript => &["js"],
            Language::Typescript => &["ts"],
            Language::Other(_) => &[],
        }
    }
}
