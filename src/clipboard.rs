use anyhow::{bail, Result};
use std::process::Command;

/// Return raw PNG bytes from the system clipboard.
pub fn get_clipboard_image() -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    return clipboard_macos();

    #[cfg(target_os = "linux")]
    return clipboard_linux();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    bail!("Unsupported platform: only macOS and Linux are supported");
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn clipboard_macos() -> Result<Vec<u8>> {
    // pngpaste writes PNG bytes to stdout when given "-"
    let out = Command::new("pngpaste")
        .arg("-")
        .output()
        .map_err(|_| anyhow::anyhow!(
            "pngpaste not found – install it with: brew install pngpaste"
        ))?;

    if !out.status.success() || out.stdout.is_empty() {
        bail!("No image found in clipboard");
    }
    Ok(out.stdout)
}

// ── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn clipboard_linux() -> Result<Vec<u8>> {
    // Try wl-paste (Wayland) first
    if let Ok(out) = Command::new("wl-paste")
        .args(["--type", "image/png", "--no-newline"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(out.stdout);
        }
    }

    // Fall back to xclip (X11)
    let out = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-o"])
        .output()
        .map_err(|_| anyhow::anyhow!(
            "No clipboard tool found – install wl-paste (Wayland) or xclip (X11)"
        ))?;

    if !out.status.success() || out.stdout.is_empty() {
        bail!("No image found in clipboard");
    }
    Ok(out.stdout)
}
