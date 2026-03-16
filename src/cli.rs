use clap::{Parser, ValueEnum};

/// Paste clipboard image as Markdown image link.
///
/// Reads an image from the clipboard, saves it to the configured backend,
/// and prints a Markdown image link to stdout.
#[derive(Parser)]
#[command(name = "mdpaste", version, about)]
pub struct Cli {
    /// Force a specific backend, overriding .mdpaste.toml
    #[arg(long, value_enum)]
    pub backend: Option<BackendChoice>,

    /// Enable debug output to stderr (shows login/cookie status)
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Clone, ValueEnum)]
pub enum BackendChoice {
    Local,
    R2,
    Nodebb,
}

impl BackendChoice {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendChoice::Local => "local",
            BackendChoice::R2 => "r2",
            BackendChoice::Nodebb => "nodebb",
        }
    }
}
