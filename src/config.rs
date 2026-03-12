use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

// ── Project config (.mdpaste.toml) ──────────────────────────────────────────

#[derive(Deserialize, Default, Debug)]
pub struct ProjectConfig {
    pub backend: Option<String>,
    pub local: Option<LocalConfig>,
    pub r2: Option<R2ProjectConfig>,
    #[allow(dead_code)]
    pub naming: Option<NamingConfig>,
}

#[derive(Deserialize, Debug)]
pub struct LocalConfig {
    pub dir: String,
}

#[derive(Deserialize, Debug)]
pub struct R2ProjectConfig {
    pub bucket: String,
    pub public_url: String,
    pub prefix: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct NamingConfig {
    #[allow(dead_code)]
    pub format: Option<String>,
}

// ── Global config (~/.config/mdpaste/config.toml) ───────────────────────────

#[derive(Deserialize, Default, Debug)]
pub struct GlobalConfig {
    pub backend: Option<String>,
    pub r2: Option<R2GlobalConfig>,
    pub wsl: Option<WslConfig>,
}

/// WSL2-specific executable paths, read from the [wsl] section of
/// ~/.config/mdpaste/config.toml.  All fields are optional; omitting them
/// causes the clipboard module to try well-known default locations.
#[derive(Deserialize, Debug)]
pub struct WslConfig {
    /// Full path to the PowerShell executable, e.g.
    /// "/mnt/c/Program Files/PowerShell/7/pwsh.exe"
    pub powershell_path: Option<String>,
    /// Full path to win32yank.exe, e.g.
    /// "/mnt/c/Users/you/AppData/Local/Microsoft/WinGet/Links/win32yank.exe"
    pub win32yank_path: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct R2GlobalConfig {
    pub account_id: String,
    pub access_key: String,
    pub secret_key: String,
    /// Override the endpoint URL (defaults to https://<account_id>.r2.cloudflarestorage.com)
    pub endpoint: Option<String>,
}

// ── Unified config ───────────────────────────────────────────────────────────

pub struct Config {
    pub project: ProjectConfig,
    pub global: GlobalConfig,
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Config {
            project: load_project_config()?,
            global: load_global_config()?,
        })
    }

    /// Resolve effective backend: CLI flag > project config > global config > "local"
    pub fn effective_backend(&self, cli_override: Option<&str>) -> String {
        cli_override
            .or(self.project.backend.as_deref())
            .or(self.global.backend.as_deref())
            .unwrap_or("local")
            .to_string()
    }
}

fn load_project_config() -> Result<ProjectConfig> {
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".mdpaste.toml");
        if candidate.exists() {
            let src = std::fs::read_to_string(&candidate)
                .with_context(|| format!("reading {}", candidate.display()))?;
            return toml::from_str(&src)
                .with_context(|| format!("parsing {}", candidate.display()));
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(ProjectConfig::default())
}

fn load_global_config() -> Result<GlobalConfig> {
    let path = global_config_path();
    if path.exists() {
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        return toml::from_str(&src).with_context(|| format!("parsing {}", path.display()));
    }
    Ok(GlobalConfig::default())
}

fn global_config_path() -> PathBuf {
    // Respect XDG_CONFIG_HOME; fall back to ~/.config
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("mdpaste").join("config.toml");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("mdpaste")
            .join("config.toml");
    }
    PathBuf::from(".mdpaste-global.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── effective_backend priority ────────────────────────────────────────────

    #[test]
    fn test_effective_backend_cli_wins() {
        let cfg = Config {
            project: ProjectConfig {
                backend: Some("r2".to_string()),
                ..Default::default()
            },
            global: GlobalConfig {
                backend: Some("r2".to_string()),
                ..Default::default()
            },
        };
        assert_eq!(cfg.effective_backend(Some("local")), "local");
    }

    #[test]
    fn test_effective_backend_project_over_global() {
        let cfg = Config {
            project: ProjectConfig {
                backend: Some("r2".to_string()),
                ..Default::default()
            },
            global: GlobalConfig {
                backend: Some("local".to_string()),
                ..Default::default()
            },
        };
        assert_eq!(cfg.effective_backend(None), "r2");
    }

    #[test]
    fn test_effective_backend_global_fallback() {
        let cfg = Config {
            project: ProjectConfig::default(),
            global: GlobalConfig {
                backend: Some("r2".to_string()),
                ..Default::default()
            },
        };
        assert_eq!(cfg.effective_backend(None), "r2");
    }

    #[test]
    fn test_effective_backend_default_local() {
        let cfg = Config {
            project: ProjectConfig::default(),
            global: GlobalConfig::default(),
        };
        assert_eq!(cfg.effective_backend(None), "local");
    }

    // ── TOML parsing ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_project_config_backend() {
        let src = r#"backend = "r2""#;
        let cfg: ProjectConfig = toml::from_str(src).unwrap();
        assert_eq!(cfg.backend, Some("r2".to_string()));
    }

    #[test]
    fn test_parse_project_config_local_section() {
        let src = r#"
[local]
dir = "assets"
"#;
        let cfg: ProjectConfig = toml::from_str(src).unwrap();
        assert_eq!(cfg.local.unwrap().dir, "assets");
    }

    #[test]
    fn test_parse_project_config_r2_section() {
        let src = r#"
[r2]
bucket = "my-bucket"
public_url = "https://cdn.example.com"
prefix = "images/"
"#;
        let cfg: ProjectConfig = toml::from_str(src).unwrap();
        let r2 = cfg.r2.unwrap();
        assert_eq!(r2.bucket, "my-bucket");
        assert_eq!(r2.public_url, "https://cdn.example.com");
        assert_eq!(r2.prefix, Some("images/".to_string()));
    }

    #[test]
    fn test_parse_global_config_backend() {
        let src = r#"backend = "local""#;
        let cfg: GlobalConfig = toml::from_str(src).unwrap();
        assert_eq!(cfg.backend, Some("local".to_string()));
    }

    #[test]
    fn test_parse_global_config_r2_credentials() {
        let src = r#"
[r2]
account_id = "abc123"
access_key = "key"
secret_key = "secret"
"#;
        let cfg: GlobalConfig = toml::from_str(src).unwrap();
        let r2 = cfg.r2.unwrap();
        assert_eq!(r2.account_id, "abc123");
        assert_eq!(r2.access_key, "key");
        assert_eq!(r2.secret_key, "secret");
        assert!(r2.endpoint.is_none());
    }

    #[test]
    fn test_parse_global_config_wsl_section() {
        let src = r#"
[wsl]
powershell_path = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
"#;
        let cfg: GlobalConfig = toml::from_str(src).unwrap();
        let wsl = cfg.wsl.unwrap();
        assert!(wsl.powershell_path.is_some());
        assert!(wsl.win32yank_path.is_none());
    }

    #[test]
    fn test_project_config_defaults_to_empty() {
        let cfg: ProjectConfig = toml::from_str("").unwrap();
        assert!(cfg.backend.is_none());
        assert!(cfg.local.is_none());
        assert!(cfg.r2.is_none());
    }

}
