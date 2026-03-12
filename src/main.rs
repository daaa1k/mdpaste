mod backend;
mod cli;
mod clipboard;
mod config;
mod markdown;
mod naming;

use anyhow::Result;
use clap::Parser;
use cli::{BackendChoice, Cli};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::load()?;

    let images = clipboard::get_clipboard_images(config.global.wsl.as_ref())?;

    let cli_backend = cli.backend.as_ref().map(|b| match b {
        BackendChoice::Local => "local",
        BackendChoice::R2 => "r2",
    });

    match config.effective_backend(cli_backend).as_str() {
        "r2" => {
            let r2_project = config
                .project
                .r2
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("[r2] section missing in .mdpaste.toml"))?;
            let r2_global = config.global.r2.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "[r2] credentials missing in global config (~/.config/mdpaste/config.toml)"
                )
            })?;
            let b = backend::r2::R2Backend::new(r2_global, r2_project).await?;
            for (i, image_data) in images.iter().enumerate() {
                let filename = filename_for(i, images.len());
                let url = b.save(image_data, &filename).await?;
                println!("{}", markdown::generate(&url));
            }
        }
        _ => {
            let dir = config
                .project
                .local
                .as_ref()
                .map(|l| l.dir.as_str())
                .unwrap_or("images");
            let b = backend::local::LocalBackend::new(dir);
            for (i, image_data) in images.iter().enumerate() {
                let filename = filename_for(i, images.len());
                let url = b.save(image_data, &filename).await?;
                println!("{}", markdown::generate(&url));
            }
        }
    }

    Ok(())
}

/// Generate a filename for the i-th image (0-based) out of `total`.
/// Single-file uploads use the plain timestamp; multi-file uploads append a 1-based index.
fn filename_for(i: usize, total: usize) -> String {
    if total == 1 {
        naming::generate_filename()
    } else {
        naming::generate_filename_n(i + 1)
    }
}
