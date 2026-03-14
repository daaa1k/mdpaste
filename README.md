# mdpaste

A CLI tool that reads an image from the clipboard, saves it as a WebP file, and outputs a Markdown image link.

## Features

- Reads images and file drops from the clipboard
- Saves images in WebP format
- Supports three storage backends:
  - **Local**: saves to a local directory (default: `images/`)
  - **R2**: uploads to Cloudflare R2 and returns a public URL
  - **NodeBB**: uploads to a NodeBB forum via the post upload API
- Backend selection: CLI flag > project config > global config > local (fallback)
- WSL2 support with configurable executable path for PowerShell

## Installation

### Homebrew (macOS / Linux)

```sh
brew tap daaa1k/tap
brew install mdpaste
```

> Supports macOS (Apple Silicon) and Linux (x86\_64).
> Intel Mac users can install via Cargo or Nix instead.

### Nix / Home Manager (recommended)

Add to your Home Manager configuration:

```nix
inputs.mdpaste.url = "github:daaa1k/mdpaste";

# in your Home Manager module:
imports = [ inputs.mdpaste.homeManagerModules.default ];

programs.mdpaste = {
  enable = true;
  # Optional: use the pre-built binary from GitHub Releases (no Rust compilation)
  # package = inputs.mdpaste.packages.${pkgs.system}.mdpaste-bin;
  settings = {
    backend = "r2";
    r2.account_id = "your-account-id";
    # R2 credentials via R2_ACCESS_KEY_ID / R2_SECRET_ACCESS_KEY env vars
  };
};
```

`programs.mdpaste.settings` is written to `$XDG_CONFIG_HOME/mdpaste/config.toml` automatically.

### Cargo

```sh
cargo install --path .
```

## Usage

```sh
mdpaste                    # use configured backend
mdpaste --backend r2       # force R2 backend
mdpaste --backend local    # force local backend
mdpaste --backend nodebb   # force NodeBB backend
```

Output example:

```
![](images/20240312_153045.webp)
```

## Configuration

### Project config (`.mdpaste.toml`)

Place this file in your project root (or any ancestor directory). Searched upward from the current directory.

```toml
backend = "r2"   # "local", "r2", or "nodebb" (optional, overrides global)

[local]
dir = "images"   # directory for local backend (default: "images")

[r2]
bucket     = "my-bucket"
public_url = "https://assets.example.com"
prefix     = "images/"   # optional key prefix

[nodebb]
url = "https://forum.example.com"
```

### Global config (`~/.config/mdpaste/config.toml`)

Stores machine-level defaults. Respects `XDG_CONFIG_HOME`.

```toml
backend = "local"   # default backend

[r2]
account_id = "..."
endpoint   = "https://..."   # optional, defaults to https://<account_id>.r2.cloudflarestorage.com

[wsl]
# Optional: specify absolute path when PowerShell is not in PATH
# (e.g., when appendWindowsPath = false in /etc/wsl.conf)
powershell_path = "/mnt/c/Program Files/PowerShell/7/pwsh.exe"
```

### R2 credentials

R2 access credentials are read from environment variables:

```sh
export R2_ACCESS_KEY_ID="your-access-key"
export R2_SECRET_ACCESS_KEY="your-secret-key"
```

### NodeBB credentials

NodeBB login credentials are read from environment variables:

```sh
export NODEBB_USERNAME="your-username"
export NODEBB_PASSWORD="your-password"
```

## WSL2 Notes

On WSL2, mdpaste uses PowerShell to access the Windows clipboard. If PowerShell is not in PATH (e.g., `appendWindowsPath = false` in `/etc/wsl.conf`), specify its absolute path in the `[wsl]` section of the global config.

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

### macOS: pngpaste

On macOS, `pngpaste` is recommended for reading clipboard images (screenshots, "Copy Image"):

```sh
brew install pngpaste
```

Without `pngpaste`, mdpaste falls back to AppleScript, which handles PNG and TIFF clipboard data.
Files copied in Finder (FileDrop) work without `pngpaste`.
