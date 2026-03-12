use anyhow::{bail, Result};
use std::process::Command;

/// Return WebP image bytes from the system clipboard.
///
/// Supports two clipboard content types:
///   - Image data (e.g. screenshots, "Copy Image" in browsers)
///   - File drop (files copied in Finder / Explorer / file managers)
///
/// The returned bytes are always WebP regardless of the source format.
pub fn get_clipboard_image() -> Result<Vec<u8>> {
    let raw = get_raw_image_bytes()?;
    convert_to_webp(&raw)
}

fn convert_to_webp(raw: &[u8]) -> Result<Vec<u8>> {
    let img =
        image::load_from_memory(raw).map_err(|e| anyhow::anyhow!("Failed to decode image: {e}"))?;
    let mut webp_data = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut webp_data),
        image::ImageFormat::WebP,
    )
    .map_err(|e| anyhow::anyhow!("Failed to encode as WebP: {e}"))?;
    Ok(webp_data)
}

fn get_raw_image_bytes() -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    return get_raw_macos();

    #[cfg(target_os = "linux")]
    return get_raw_linux();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    bail!("Unsupported platform: only macOS and Linux are supported");
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn get_raw_macos() -> Result<Vec<u8>> {
    // pngpaste reads image data from clipboard and outputs PNG bytes to stdout.
    if let Ok(out) = Command::new("pngpaste").arg("-").output() {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(out.stdout);
        }
    }

    // FileDrop: try to read a file path from the clipboard via AppleScript.
    let out = Command::new("osascript")
        .args([
            "-e",
            "try",
            "-e",
            "POSIX path of (the clipboard as \u{ab}class furl\u{bb})",
            "-e",
            "on error",
            "-e",
            "\"\"",
            "-e",
            "end try",
        ])
        .output()
        .map_err(|_| anyhow::anyhow!("osascript not found"))?;

    if out.status.success() {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return std::fs::read(&path)
                .map_err(|e| anyhow::anyhow!("Failed to read file '{path}': {e}"));
        }
    }

    bail!("No image found in clipboard");
}

// ── Linux / WSL2 ─────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn get_raw_linux() -> Result<Vec<u8>> {
    if is_wsl() {
        return get_raw_wsl();
    }

    // Try image data via Wayland (multiple MIME types in order of preference).
    for mime in &["image/png", "image/jpeg", "image/gif", "image/webp"] {
        if let Ok(out) = Command::new("wl-paste")
            .args(["--type", mime, "--no-newline"])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(out.stdout);
            }
        }
    }

    // Try image data via X11.
    for mime in &["image/png", "image/jpeg", "image/gif", "image/webp"] {
        if let Ok(out) = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", mime, "-o"])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(out.stdout);
            }
        }
    }

    // FileDrop: try text/uri-list via Wayland.
    if let Ok(out) = Command::new("wl-paste")
        .args(["--type", "text/uri-list", "--no-newline"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            let uris = String::from_utf8_lossy(&out.stdout);
            if let Some(path) = parse_first_file_uri(&uris) {
                return std::fs::read(&path)
                    .map_err(|e| anyhow::anyhow!("Failed to read file '{path}': {e}"));
            }
        }
    }

    // FileDrop: try text/uri-list via X11.
    if let Ok(out) = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "text/uri-list", "-o"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            let uris = String::from_utf8_lossy(&out.stdout);
            if let Some(path) = parse_first_file_uri(&uris) {
                return std::fs::read(&path)
                    .map_err(|e| anyhow::anyhow!("Failed to read file '{path}': {e}"));
            }
        }
    }

    bail!("No image found in clipboard – install wl-paste (Wayland) or xclip (X11)");
}

#[cfg(target_os = "linux")]
fn get_raw_wsl() -> Result<Vec<u8>> {
    // Try PowerShell: read image data from Windows clipboard and stream as PNG bytes.
    let ps_image = concat!(
        "Add-Type -Assembly System.Windows.Forms;",
        "$img=[System.Windows.Forms.Clipboard]::GetImage();",
        "if($img -eq $null){exit 1};",
        "$ms=New-Object System.IO.MemoryStream;",
        "$img.Save($ms,[System.Drawing.Imaging.ImageFormat]::Png);",
        "$b=$ms.ToArray();",
        "[Console]::OpenStandardOutput().Write($b,0,$b.Length)"
    );
    if let Ok(out) = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_image])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(out.stdout);
        }
    }

    // FileDrop: get first Windows file path from clipboard, then convert to WSL path.
    let ps_files = concat!(
        "Add-Type -Assembly System.Windows.Forms;",
        "$files=[System.Windows.Forms.Clipboard]::GetFileDropList();",
        "if($files -eq $null -or $files.Count -eq 0){exit 1};",
        "Write-Output $files[0]"
    );
    if let Ok(out) = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_files])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            let win_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !win_path.is_empty() {
                let wsl_out = Command::new("wslpath")
                    .args(["-u", &win_path])
                    .output()
                    .map_err(|_| {
                        anyhow::anyhow!("wslpath not found – ensure WSL2 interop is enabled")
                    })?;
                if wsl_out.status.success() {
                    let wsl_path = String::from_utf8_lossy(&wsl_out.stdout).trim().to_string();
                    return std::fs::read(&wsl_path)
                        .map_err(|e| anyhow::anyhow!("Failed to read file '{wsl_path}': {e}"));
                }
            }
        }
    }

    // Fallback: win32yank.exe (verify PNG magic bytes).
    if let Ok(out) = Command::new("win32yank.exe").arg("-o").output() {
        if out.status.success() && out.stdout.starts_with(b"\x89PNG") {
            return Ok(out.stdout);
        }
    }

    bail!(
        "No image found in Windows clipboard (powershell.exe failed and win32yank.exe not found)"
    );
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

/// Parse the first `file://` URI from a `text/uri-list` payload and return the
/// decoded filesystem path.  Lines starting with `#` are comments (RFC 2483).
fn parse_first_file_uri(uris: &str) -> Option<String> {
    for line in uris.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("file://") {
            // rest may be "/path" (empty authority) or "hostname/path"
            let path = if rest.starts_with('/') {
                rest.to_string()
            } else {
                // skip the authority component
                rest.splitn(2, '/').nth(1).map(|p| format!("/{p}"))?
            };
            return Some(url_decode(&path));
        }
    }
    None
}

/// Percent-decode a URI path component.
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hi_lo) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hi_lo, 16) {
                    result.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
