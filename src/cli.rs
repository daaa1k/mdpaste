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
}

#[derive(Clone, ValueEnum)]
pub enum BackendChoice {
    Local,
    R2,
    Nodebb,
}
