//! Puts the transcript into whatever app has focus.
//!
//! We paste rather than type key by key: pasting is instant, works with any
//! keyboard layout, and handles accents and emoji. The user's clipboard is put
//! back afterwards so dictation doesn't overwrite what they copied.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// `on_main` runs a job on the app's main thread and waits for it. macOS only
/// allows keyboard-layout lookups (needed to press ⌘V) on the main thread.
pub fn paste(
    text: &str,
    restore_clipboard: bool,
    on_main: impl FnOnce(Box<dyn FnOnce() -> Result<()> + Send>) -> Result<()>,
) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| anyhow!("Clipboard unavailable: {e}"))?;
    let previous = if restore_clipboard { clipboard.get_text().ok() } else { None };

    clipboard
        .set_text(text.to_string())
        .map_err(|e| anyhow!("Could not write to the clipboard: {e}"))?;
    // Give the system a moment to publish the new clipboard contents.
    sleep(Duration::from_millis(40));

    on_main(Box::new(press_paste))?;

    if let Some(old) = previous {
        // Wait for the target app to read the clipboard before restoring it.
        sleep(Duration::from_millis(250));
        let _ = clipboard.set_text(old);
    }
    Ok(())
}

/// Puts text on the clipboard without pasting it.
pub fn copy(text: &str) -> Result<()> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_string()))
        .map_err(|e| anyhow!("Could not write to the clipboard: {e}"))
}

pub fn press_paste() -> Result<()> {
    // Never let the keyboard library pop up macOS's permission dialog here;
    // Parla checks the permission itself and explains what to do.
    let settings = Settings { open_prompt_to_get_permissions: false, ..Settings::default() };
    let mut enigo = Enigo::new(&settings).map_err(|e| anyhow!("Keyboard control unavailable: {e}"))?;
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo.key(modifier, Direction::Press)?;
    let result = enigo.key(Key::Unicode('v'), Direction::Click);
    enigo.key(modifier, Direction::Release)?;
    result?;
    Ok(())
}

/// Whether macOS lets Parla send keystrokes to other apps (Accessibility).
#[cfg(target_os = "macos")]
pub fn can_control_keyboard() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
pub fn can_control_keyboard() -> bool {
    true
}
