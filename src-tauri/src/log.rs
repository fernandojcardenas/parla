//! Tiny log: prints to the terminal and appends to ~/Library/Logs/Parla/parla.log,
//! so problems can be diagnosed after the fact.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

fn path() -> &'static Option<PathBuf> {
    LOG_PATH.get_or_init(|| {
        let home = std::env::var_os("HOME")?;
        let dir = PathBuf::from(home).join("Library/Logs/Parla");
        fs::create_dir_all(&dir).ok()?;
        Some(dir.join("parla.log"))
    })
}

pub fn write(msg: &str) {
    eprintln!("[parla] {msg}");
    if let Some(p) = path() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(f, "{secs} {msg}");
        }
    }
}

#[macro_export]
macro_rules! plog {
    ($($arg:tt)*) => { $crate::log::write(&format!($($arg)*)) };
}
