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
        .map_err(|_| {
            anyhow::anyhow!("pngpaste not found – install it with: brew install pngpaste")
        })?;

    if !out.status.success() || out.stdout.is_empty() {
        bail!("No image found in clipboard");
    }
    Ok(out.stdout)
}

// ── Linux / WSL2 ─────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn clipboard_linux() -> Result<Vec<u8>> {
    if is_wsl() {
        return clipboard_wsl();
    }

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
        .map_err(|_| {
            anyhow::anyhow!("No clipboard tool found – install wl-paste (Wayland) or xclip (X11)")
        })?;

    if !out.status.success() || out.stdout.is_empty() {
        bail!("No image found in clipboard");
    }
    Ok(out.stdout)
}

/// Detect WSL2 by checking well-known environment variables and /proc/version.
#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    if std::env::var("WSL_DISTRO_NAME").is_ok() || std::env::var("WSLENV").is_ok() {
        return true;
    }
    std::fs::read_to_string("/proc/version")
        .map(|v| {
            let lower = v.to_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        })
        .unwrap_or(false)
}

/// WSL2: use PowerShell to extract a PNG from the Windows clipboard and
/// stream the raw bytes to stdout.
///
/// Requires powershell.exe accessible from WSL (standard on WSL2).
/// win32yank.exe is used as an alternative if available and powershell fails,
/// but PowerShell is preferred for binary image data.
#[cfg(target_os = "linux")]
fn clipboard_wsl() -> Result<Vec<u8>> {
    // PowerShell one-liner: read clipboard image, encode as PNG, write raw bytes to stdout.
    let ps_script = concat!(
        "Add-Type -Assembly System.Windows.Forms;",
        "$img=[System.Windows.Forms.Clipboard]::GetImage();",
        "if($img -eq $null){exit 1};",
        "$ms=New-Object System.IO.MemoryStream;",
        "$img.Save($ms,[System.Drawing.Imaging.ImageFormat]::Png);",
        "$b=$ms.ToArray();",
        "[Console]::OpenStandardOutput().Write($b,0,$b.Length)"
    );

    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .output()
        .map_err(|_| anyhow::anyhow!("powershell.exe not found – ensure WSL2 interop is enabled"))?;

    if out.status.success() && !out.stdout.is_empty() {
        return Ok(out.stdout);
    }

    // Fallback: win32yank.exe (handles text natively; for images it outputs raw clipboard bytes)
    let out = Command::new("win32yank.exe")
        .arg("-o")
        .output()
        .map_err(|_| anyhow::anyhow!("No image in Windows clipboard (powershell.exe failed and win32yank.exe not found)"))?;

    if out.status.success() && !out.stdout.is_empty() {
        // win32yank outputs raw clipboard data; verify it looks like a PNG (magic bytes 89 50 4E 47)
        if out.stdout.starts_with(b"\x89PNG") {
            return Ok(out.stdout);
        }
    }

    bail!("No image found in Windows clipboard");
}
