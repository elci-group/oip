// Copyright (c) 2026 elci-group
// SPDX-License-Identifier: MIT
// -----------------------------
// src/main.rs
// -----------------------------

use arboard::Clipboard;
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

mod languages;
use crate::languages::Language;

/// Suffix appended (not substituted) to a target's full file name to derive
/// its backup path, and stripped (not substituted) to invert it. Using a
/// suffix rather than `Path::with_extension` keeps `backup_path_for` and
/// `original_path_for` exact inverses of each other, regardless of how many
/// dots are in the original file name.
const BACKUP_SUFFIX: &str = ".oip.backup";

fn backup_path_for(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(BACKUP_SUFFIX);
    PathBuf::from(name)
}

fn original_path_for(backup: &Path) -> Option<PathBuf> {
    let s = backup.as_os_str().to_str()?;
    s.strip_suffix(BACKUP_SUFFIX).map(PathBuf::from)
}

#[derive(Parser)]
#[command(
    name = "oip",
    version = "0.1.0",
    author = "You",
    about = "Overwrite In Place with Waffle Splitting and Temporal Tethering"
)]
struct Cli {
    /// Target file to overwrite
    target: Option<PathBuf>,

    /// Interactive / Smart mode
    #[arg(short, long)]
    interactive: bool,

    /// Revert last change
    #[arg(long)]
    revert: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = if cli.revert {
        revert_last_change()
    } else if cli.interactive {
        run_interactive_mode()
    } else if let Some(target) = cli.target {
        apply_clipboard_to_file(&target)
    } else {
        Err("No target file provided".to_string())
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the clipboard and overwrites `target` with its text content. A
/// clipboard that can't be read as text (empty, an image, a read error) is
/// treated as "nothing to apply" and leaves `target` untouched rather than
/// being written as an empty string — the previous behavior silently wiped
/// the target file whenever the clipboard read failed.
fn apply_clipboard_to_file(target: &PathBuf) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
    let text = clipboard.get_text().map_err(|e| {
        format!("Could not read clipboard text ({e}); leaving {target:?} untouched")
    })?;

    if text.is_empty() {
        return Err(format!("Clipboard is empty; leaving {target:?} untouched"));
    }

    let _lang = Language::from_text(&text);

    let previous = fs::read_to_string(target).ok();
    print_diff_preview(target, previous.as_deref(), &text);

    backup_file(target)?;
    fs::write(target, &text).map_err(|e| format!("Write to {target:?} failed: {e}"))?;
    println!("Applied clipboard to {target:?}");
    Ok(())
}

/// Prints a short before/after summary so a change is visible before it
/// lands on disk, per oip's documented "diff preview before overwriting".
fn print_diff_preview(target: &Path, previous: Option<&str>, next: &str) {
    match previous {
        None => println!("{target:?}: new file, {} line(s)", next.lines().count()),
        Some(prev) if prev == next => {
            println!("{target:?}: clipboard matches current content, no change")
        }
        Some(prev) => {
            let old_lines: Vec<&str> = prev.lines().collect();
            let new_lines: Vec<&str> = next.lines().collect();
            println!(
                "{target:?}: {} line(s) -> {} line(s)",
                old_lines.len(),
                new_lines.len()
            );
            let common_prefix = old_lines
                .iter()
                .zip(new_lines.iter())
                .take_while(|(a, b)| a == b)
                .count();
            if let Some(line) = old_lines.get(common_prefix) {
                println!("- {line}");
            }
            if let Some(line) = new_lines.get(common_prefix) {
                println!("+ {line}");
            }
        }
    }
}

fn run_interactive_mode() -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
    let mut last = String::new();
    let mut last_change = Instant::now();

    loop {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(current) = clipboard.get_text() {
            if current != last {
                last = current.clone();
                last_change = Instant::now();
                println!("Clipboard updated. Waiting for stability...");
            }
        }

        if last_change.elapsed() > Duration::from_secs(2) && !last.is_empty() {
            let lang = Language::from_text(&last);
            return match infer_target_file(&lang) {
                Some(target) => {
                    println!("Applying to {target:?}");
                    apply_clipboard_to_file(&target)
                }
                None => Err(format!(
                    "No candidate {} file found under the project root; pass a target path explicitly",
                    lang.name()
                )),
            };
        }
    }
}

/// Locates the project root by walking up from the current directory until a
/// `Cargo.toml` or `package.json` is found (falling back to the current
/// directory), then searches under it for the most recently modified file
/// whose extension matches the clipboard's detected language. Files under
/// `target/`, `node_modules/`, and `.git/` are skipped.
fn infer_target_file(lang: &Language) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let root = find_project_root(&cwd).unwrap_or(cwd);
    find_candidate_file(&root, lang)
}

/// Most recently modified file under `root` whose extension matches `lang`.
/// Factored out of `infer_target_file` so it can be tested against a temp
/// directory instead of the process's real working directory.
fn find_candidate_file(root: &Path, lang: &Language) -> Option<PathBuf> {
    let extensions = lang.extensions();
    if extensions.is_empty() {
        return None;
    }

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if matches!(name.as_ref(), "target" | "node_modules" | ".git") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let matches_ext = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.contains(&e));
            if !matches_ext {
                continue;
            }
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, path));
            }
        }
    }
    best.map(|(_, path)| path)
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join("Cargo.toml").is_file() || dir.join("package.json").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn backup_file(target: &Path) -> Result<(), String> {
    match fs::read_to_string(target) {
        Ok(contents) => {
            let backup = backup_path_for(target);
            fs::write(&backup, contents)
                .map_err(|e| format!("Could not write backup {backup:?}: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), // new file, nothing to back up
        Err(e) => Err(format!(
            "Could not read {target:?} to back it up ({e}); refusing to overwrite it without a backup"
        )),
    }
}

fn revert_last_change() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not read cwd: {e}"))?;
    revert_in_dir(&cwd)
}

/// Restores every `*.oip.backup` file under `dir` to its original path.
/// Factored out of `revert_last_change` so the round trip with
/// `backup_file`/`backup_path_for`/`original_path_for` can be tested against
/// a temp directory instead of the process's real working directory.
fn revert_in_dir(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Could not read {dir:?}: {e}"))?;

    let mut reverted = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(orig) = original_path_for(&path) else {
            continue;
        };
        match fs::copy(&path, &orig) {
            Ok(_) => {
                println!("Reverted {orig:?}");
                reverted += 1;
            }
            Err(e) => eprintln!("Could not revert {orig:?} from {path:?}: {e}"),
        }
    }

    if reverted == 0 {
        return Err(format!("No {BACKUP_SUFFIX} backups found in {dir:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh, unique scratch directory per test so parallel test runs
    /// don't collide.
    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "oip-test-{}-{}-{n}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn backup_and_original_path_round_trip() {
        for name in ["main.rs", "notes.txt", "archive.tar.gz", "Makefile"] {
            let target = PathBuf::from(name);
            let backup = backup_path_for(&target);
            assert_eq!(
                original_path_for(&backup).as_deref(),
                Some(target.as_path())
            );
        }
    }

    #[test]
    fn unrelated_file_has_no_original_path() {
        assert_eq!(original_path_for(Path::new("main.rs")), None);
    }

    // Regression test for the bug this suffix scheme replaced: the old
    // `target.with_extension("oip.backup")` / `path.with_extension("")`
    // pair produced "main.oip.backup" -> reverted to "main.oip", never
    // restoring the original "main.rs". `--revert` silently no-opped on
    // the real file while reporting success.
    #[test]
    fn apply_then_revert_restores_original_content() {
        let dir = temp_dir();
        let target = dir.join("main.rs");
        fs::write(&target, "original content").unwrap();

        backup_file(&target).unwrap();
        fs::write(&target, "clobbered content").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "clobbered content");

        revert_in_dir(&dir).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");
    }

    #[test]
    fn revert_in_dir_errors_when_no_backups_present() {
        let dir = temp_dir();
        fs::write(dir.join("main.rs"), "content").unwrap();
        assert!(revert_in_dir(&dir).is_err());
    }

    #[test]
    fn backup_file_leaves_missing_target_untouched() {
        let dir = temp_dir();
        let target = dir.join("does-not-exist.rs");
        assert!(backup_file(&target).is_ok());
        assert!(!backup_path_for(&target).exists());
    }

    #[test]
    fn infer_target_file_finds_recently_modified_matching_extension() {
        let dir = temp_dir();
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "fn lib() {}").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let newest = src.join("main.rs");
        fs::write(&newest, "fn main() {}").unwrap();

        let root = find_project_root(&src).unwrap();
        assert_eq!(root, dir);

        assert_eq!(find_candidate_file(&root, &Language::Rust), Some(newest));
    }

    #[test]
    fn find_candidate_file_returns_none_for_language_without_extensions() {
        let dir = temp_dir();
        fs::write(dir.join("notes.txt"), "hello").unwrap();
        assert_eq!(
            find_candidate_file(&dir, &Language::Other("txt".into())),
            None
        );
    }
}
