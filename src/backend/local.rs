use anyhow::Result;
use std::path::Path;

pub struct LocalBackend {
    dir: String,
}

impl LocalBackend {
    pub fn new(dir: &str) -> Self {
        LocalBackend { dir: dir.to_string() }
    }

    /// Save `image` bytes to `<dir>/<filename>` and return a relative Markdown URL.
    pub async fn save(&self, image: &[u8], filename: &str) -> Result<String> {
        std::fs::create_dir_all(&self.dir)?;
        let path = Path::new(&self.dir).join(filename);
        std::fs::write(&path, image)?;
        // Always use forward slashes so the Markdown link is portable
        Ok(format!("./{}/{}", self.dir, filename))
    }
}
