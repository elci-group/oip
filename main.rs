// -----------------------------
// src/main.rs
// -----------------------------

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use clap::Parser;
use arboard::Clipboard;

mod languages;
use crate::languages::Language;

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

fn main() {
    let cli = Cli::parse();

    if cli.revert {
        revert_last_change();
        return;
    }

    if cli.interactive {
        run_interactive_mode();
        return;
    }

    if let Some(target) = cli.target {
        apply_clipboard_to_file(&target);
    } else {
        eprintln!("No target file provided");
    }
}

fn apply_clipboard_to_file(target: &PathBuf) {
    let mut clipboard = Clipboard::new().expect("Clipboard unavailable");
    let text = clipboard.get_text().unwrap_or_default();

    let _lang = Language::from_text(&text);
    let processed = text;

    backup_file(target);
    fs::write(target, processed).expect("Write failed");
}

fn run_interactive_mode() {
    let mut clipboard = Clipboard::new().expect("Clipboard unavailable");
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
            if let Some(target) = infer_target_file() {
                println!("Applying to {:?}", target);
                apply_clipboard_to_file(&target);
                return;
            }
        }
    }
}

fn infer_target_file() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut path = cwd.clone();

    // If running from target/debug or target/release, go up 3 dirs to project root
    if path.ends_with("release") || path.ends_with("debug") {
        path = path.parent()?.parent()?.parent()?.to_path_buf();
    }

    Some(path.join("src/main.rs"))
}

fn backup_file(target: &PathBuf) {
    if let Ok(contents) = fs::read_to_string(target) {
        let backup = target.with_extension("oip.backup");
        let _ = fs::write(&backup, contents);
    }
}

fn revert_last_change() {
    let cwd = std::env::current_dir().expect("cwd");
    for entry in fs::read_dir(cwd).expect("read_dir failed") {
        let entry = entry.unwrap();
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "backup" || ext == "oip.backup" {
                let orig = path.with_extension("");
                let _ = fs::copy(&path, &orig);
                println!("Reverted {:?}", &orig);
            }
        }
    }
}
