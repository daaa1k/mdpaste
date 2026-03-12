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

    let image_data = clipboard::get_clipboard_image(config.global.wsl.as_ref())?;
    let filename = naming::generate_filename();

    let cli_backend = cli.backend.as_ref().map(|b| match b {
        BackendChoice::Local => "local",
        BackendChoice::R2 => "r2",
    });

    let url = match config.effective_backend(cli_backend).as_str() {
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
            b.save(&image_data, &filename).await?
        }
        _ => {
            let dir = config
                .project
                .local
                .as_ref()
                .map(|l| l.dir.as_str())
                .unwrap_or("images");
            let b = backend::local::LocalBackend::new(dir);
            b.save(&image_data, &filename).await?
        }
    };

    println!("{}", markdown::generate(&url));
    Ok(())
}
