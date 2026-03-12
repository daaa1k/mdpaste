# mdpaste

A CLI tool that reads an image from the clipboard, saves it as a WebP file, and outputs a Markdown image link.

## Features

- Reads images and file drops from the clipboard
- Saves images in WebP format
- Supports two storage backends:
  - **Local**: saves to a local directory (default: `images/`)
  - **R2**: uploads to Cloudflare R2 and returns a public URL
- Backend selection: CLI flag > project config > global config > local (fallback)
- WSL2 support with configurable executable paths for PowerShell and win32yank

## Installation

```sh
cargo install --path .
```

## Usage

```sh
mdpaste              # use configured backend
mdpaste --backend r2     # force R2 backend
mdpaste --backend local  # force local backend
```

Output example:

```
![](images/20240312_153045.webp)
```

## Configuration

### Project config (`.mdpaste.toml`)

Place this file in your project root (or any ancestor directory). Searched upward from the current directory.

```toml
backend = "r2"   # "local" or "r2" (optional, overrides global)

[local]
dir = "images"   # directory for local backend (default: "images")

[r2]
bucket     = "my-bucket"
public_url = "https://assets.example.com"
prefix     = "images/"   # optional key prefix
```

### Global config (`~/.config/mdpaste/config.toml`)

Stores credentials and machine-level defaults. Respects `XDG_CONFIG_HOME`.

```toml
backend = "local"   # default backend

[r2]
account_id = "..."
access_key = "..."
secret_key = "..."
endpoint   = "https://..."   # optional, defaults to https://<account_id>.r2.cloudflarestorage.com

[wsl]
# Optional: specify absolute paths when Windows executables are not in PATH
# (e.g., when appendWindowsPath = false in /etc/wsl.conf)
powershell_path = "/mnt/c/Program Files/PowerShell/7/pwsh.exe"
win32yank_path  = "/mnt/c/Users/you/AppData/Local/Microsoft/WinGet/Links/win32yank.exe"
```

## WSL2 Notes

On WSL2, mdpaste uses PowerShell to access the Windows clipboard. If Windows executables are not in PATH (e.g., `appendWindowsPath = false` in `/etc/wsl.conf`), specify their absolute paths in the `[wsl]` section of the global config.

PowerShell resolution order:
1. `powershell_path` from `[wsl]` config
2. `powershell.exe` / `pwsh.exe` in PATH
3. Well-known paths under `/mnt/c/`

## Platform Support

| Platform | Image | FileDrop |
|----------|-------|---------|
| macOS    | ✅    | ✅      |
| Linux    | ✅    | ✅      |
| WSL2     | ✅    | ✅      |
