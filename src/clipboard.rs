use anyhow::{bail, Result};
use std::process::Command;

use crate::config::WslConfig;

/// An image obtained from the clipboard, together with its file extension.
///
/// - Clipboard image data (screenshot, "Copy Image") → WebP-encoded bytes, `extension = "webp"`
/// - FileDrop (files copied in a file manager) → raw file bytes, extension from the source file
pub struct ClipboardImage {
    pub data: Vec<u8>,
    /// Lowercase file extension without a leading dot, e.g. `"webp"`, `"png"`, `"gif"`.
    pub extension: String,
}

/// Return all images/files from the system clipboard as [`ClipboardImage`] entries.
///
/// Clipboard image data is converted to WebP.  FileDrop files are returned as-is so that
/// animated WebP (and other formats) are preserved.
pub fn get_clipboard_images(wsl_config: Option<&WslConfig>) -> Result<Vec<ClipboardImage>> {
    #[cfg(target_os = "macos")]
    {
        let _ = wsl_config;
        get_images_macos()
    }

    #[cfg(target_os = "linux")]
    return get_images_linux(wsl_config);

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    bail!("Unsupported platform: only macOS and Linux are supported");
}

/// Convert raw image bytes (any format supported by the `image` crate) to WebP.
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

/// Wrap clipboard image data as a WebP [`ClipboardImage`].
fn clipboard_webp(raw: &[u8]) -> Result<ClipboardImage> {
    let data = convert_to_webp(raw)?;
    Ok(ClipboardImage {
        data,
        extension: "webp".to_string(),
    })
}

/// Read a file and return it as a [`ClipboardImage`], preserving the original extension.
fn file_image(path: &str) -> Result<ClipboardImage> {
    let data =
        std::fs::read(path).map_err(|e| anyhow::anyhow!("Failed to read file '{path}': {e}"))?;
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("webp")
        .to_lowercase();
    Ok(ClipboardImage { data, extension })
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn get_images_macos() -> Result<Vec<ClipboardImage>> {
    // pngpaste reads image data from clipboard and outputs PNG bytes to stdout.
    if let Ok(out) = Command::new("pngpaste").arg("-").output() {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(vec![clipboard_webp(&out.stdout)?]);
        }
    }

    // FileDrop: read all file paths from the clipboard via AppleScript.
    // `{«class furl»}` coercion returns a list even when a single file is copied.
    let out = Command::new("osascript")
        .args([
            "-e",
            "set output to \"\"",
            "-e",
            "try",
            "-e",
            "set fileList to (the clipboard as {\u{ab}class furl\u{bb}})",
            "-e",
            "repeat with f in fileList",
            "-e",
            "set output to output & POSIX path of f & linefeed",
            "-e",
            "end repeat",
            "-e",
            "on error",
            "-e",
            "end try",
            "-e",
            "output",
        ])
        .output()
        .map_err(|_| anyhow::anyhow!("osascript not found"))?;

    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout);
        let paths: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if !paths.is_empty() {
            return paths.iter().map(|p| file_image(p)).collect();
        }
    }

    bail!("No image found in clipboard");
}

// ── Linux / WSL2 ─────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn get_images_linux(wsl_config: Option<&WslConfig>) -> Result<Vec<ClipboardImage>> {
    if is_wsl() {
        return get_images_wsl(wsl_config);
    }

    // Try image data via Wayland (multiple MIME types in order of preference).
    for mime in &["image/png", "image/jpeg", "image/gif", "image/webp"] {
        if let Ok(out) = Command::new("wl-paste")
            .args(["--type", mime, "--no-newline"])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(vec![clipboard_webp(&out.stdout)?]);
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
                return Ok(vec![clipboard_webp(&out.stdout)?]);
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
            let paths = parse_all_file_uris(&uris);
            if !paths.is_empty() {
                return paths.iter().map(|p| file_image(p)).collect();
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
            let paths = parse_all_file_uris(&uris);
            if !paths.is_empty() {
                return paths.iter().map(|p| file_image(p)).collect();
            }
        }
    }

    bail!("No image found in clipboard – install wl-paste (Wayland) or xclip (X11)");
}

#[cfg(target_os = "linux")]
fn get_images_wsl(wsl_config: Option<&WslConfig>) -> Result<Vec<ClipboardImage>> {
    let ps = resolve_powershell(wsl_config);

    if let Some(ref ps) = ps {
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
        if let Ok(out) = Command::new(ps)
            .args([
                "-Sta",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                ps_image,
            ])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(vec![clipboard_webp(&out.stdout)?]);
            }
        }

        // FileDrop: get all Windows file paths from clipboard, then convert to WSL paths.
        // OutputEncoding must be set to UTF-8 to handle non-ASCII paths correctly.
        // -Sta is required for System.Windows.Forms.Clipboard (STA apartment model).
        let ps_files = concat!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8;",
            "Add-Type -Assembly System.Windows.Forms;",
            "$files=[System.Windows.Forms.Clipboard]::GetFileDropList();",
            "if($files -eq $null -or $files.Count -eq 0){exit 1};",
            "foreach($f in $files){Write-Output $f}"
        );
        if let Ok(out) = Command::new(ps)
            .args([
                "-Sta",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                ps_files,
            ])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                let output = String::from_utf8_lossy(&out.stdout);
                let win_paths: Vec<&str> = output
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect();
                if !win_paths.is_empty() {
                    let mut images = Vec::new();
                    for win_path in win_paths {
                        let wsl_out = Command::new("wslpath")
                            .args(["-u", win_path])
                            .output()
                            .map_err(|_| {
                                anyhow::anyhow!(
                                    "wslpath not found – ensure WSL2 interop is enabled"
                                )
                            })?;
                        if wsl_out.status.success() {
                            let wsl_path =
                                String::from_utf8_lossy(&wsl_out.stdout).trim().to_string();
                            images.push(file_image(&wsl_path)?);
                        }
                    }
                    if !images.is_empty() {
                        return Ok(images);
                    }
                }
            }
        }
    }

    // Fallback: win32yank.exe (verify PNG magic bytes).
    let win32yank = wsl_config
        .and_then(|c| c.win32yank_path.as_deref())
        .unwrap_or("win32yank.exe");
    if let Ok(out) = Command::new(win32yank).arg("-o").output() {
        if out.status.success() && out.stdout.starts_with(b"\x89PNG") {
            return Ok(vec![clipboard_webp(&out.stdout)?]);
        }
    }

    bail!(
        "No image found in Windows clipboard \
         (PowerShell not found or clipboard is empty; win32yank.exe not found). \
         Configure [wsl] powershell_path and win32yank_path in \
         ~/.config/mdpaste/config.toml"
    );
}

/// Find a usable PowerShell executable.
///
/// Resolution order:
///   1. Configured path from [wsl] powershell_path in ~/.config/mdpaste/config.toml
///   2. PATH lookup (works when `appendWindowsPath = true` in /etc/wsl.conf)
///   3. Well-known Windows paths via the WSL2 filesystem mount
#[cfg(target_os = "linux")]
fn resolve_powershell(wsl_config: Option<&WslConfig>) -> Option<std::ffi::OsString> {
    // 1. Configured path
    if let Some(path) = wsl_config.and_then(|c| c.powershell_path.as_deref()) {
        if std::path::Path::new(path).exists() {
            return Some(path.into());
        }
    }
    // 2. PATH lookup
    for name in &["powershell.exe", "pwsh.exe"] {
        if Command::new(name)
            .args(["-NoProfile", "-Command", "exit 0"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some((*name).into());
        }
    }
    // 3. Well-known Windows paths accessible through the WSL2 filesystem mount
    let candidates = [
        "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
        "/mnt/c/Program Files/PowerShell/7/pwsh.exe",
        "/mnt/c/Program Files/PowerShell/pwsh.exe",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some((*path).into());
        }
    }
    None
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

/// Parse all `file://` URIs from a `text/uri-list` payload and return the
/// decoded filesystem paths.  Lines starting with `#` are comments (RFC 2483).
#[cfg(target_os = "linux")]
fn parse_all_file_uris(uris: &str) -> Vec<String> {
    let mut paths = Vec::new();
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
                match rest.split_once('/') {
                    Some(x) => format!("/{}", x.1),
                    None => continue,
                }
            };
            paths.push(url_decode(&path));
        }
    }
    paths
}

/// Percent-decode a URI path component.
#[cfg(target_os = "linux")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_webp_from_png() {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([255u8, 0, 0, 255]));
        let mut png_data = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_data),
            image::ImageFormat::Png,
        )
        .unwrap();

        let webp = convert_to_webp(&png_data).unwrap();
        // WebP files start with RIFF and have WEBP at offset 8
        assert!(webp.starts_with(b"RIFF"));
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn test_convert_to_webp_invalid_input() {
        let result = convert_to_webp(b"not an image");
        assert!(result.is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_all_file_uris_basic() {
        let uris = "file:///home/user/image.png\nfile:///home/user/photo.jpg\n";
        let paths = parse_all_file_uris(uris);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/home/user/image.png");
        assert_eq!(paths[1], "/home/user/photo.jpg");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_all_file_uris_comments_and_empty() {
        let uris = "# comment\nfile:///tmp/test.webp\n\n# another comment\n";
        let paths = parse_all_file_uris(uris);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "/tmp/test.webp");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_all_file_uris_with_authority() {
        let uris = "file://localhost/home/user/test.png\n";
        let paths = parse_all_file_uris(uris);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "/home/user/test.png");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_all_file_uris_empty() {
        assert!(parse_all_file_uris("").is_empty());
        assert!(parse_all_file_uris("# only comments\n").is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_url_decode_plain() {
        assert_eq!(url_decode("/home/user/file.png"), "/home/user/file.png");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_url_decode_space() {
        assert_eq!(
            url_decode("/home/user/my%20file.png"),
            "/home/user/my file.png"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_url_decode_unicode() {
        // %E6%97%A5 is the UTF-8 encoding of '日'
        let decoded = url_decode("/tmp/%E6%97%A5.png");
        assert_eq!(decoded, "/tmp/日.png");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_url_decode_incomplete_percent() {
        // Trailing % without two hex digits should be kept as-is
        let decoded = url_decode("/tmp/file%");
        assert_eq!(decoded, "/tmp/file%");
    }

    #[test]
    fn test_file_image_extension() {
        let path = std::env::temp_dir().join("mdpaste_test_ext.gif");
        std::fs::write(&path, b"GIF89a").unwrap();
        let img = file_image(path.to_str().unwrap()).unwrap();
        assert_eq!(img.extension, "gif");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_file_image_uppercase_extension() {
        let path = std::env::temp_dir().join("mdpaste_test_ext.WEBP");
        std::fs::write(&path, b"RIFF....WEBP").unwrap();
        let img = file_image(path.to_str().unwrap()).unwrap();
        assert_eq!(img.extension, "webp");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_file_image_no_extension_defaults_to_webp() {
        let path = std::env::temp_dir().join("mdpaste_test_noext");
        std::fs::write(&path, b"data").unwrap();
        let img = file_image(path.to_str().unwrap()).unwrap();
        assert_eq!(img.extension, "webp");
        let _ = std::fs::remove_file(&path);
    }
}
