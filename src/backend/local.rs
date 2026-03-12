use anyhow::Result;
use std::path::Path;

pub struct LocalBackend {
    dir: String,
}

impl LocalBackend {
    pub fn new(dir: &str) -> Self {
        LocalBackend {
            dir: dir.to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_creates_file_and_returns_url() {
        let dir = std::env::temp_dir().join("mdpaste_local_test");
        let dir_str = dir.to_str().unwrap().to_string();
        let backend = LocalBackend::new(&dir_str);

        let data = b"fake webp data";
        let url = backend.save(data, "test.webp").await.unwrap();

        assert!(
            url.contains("test.webp"),
            "URL should contain filename: {url}"
        );
        assert!(url.starts_with("./"), "URL should be relative: {url}");

        let file_path = dir.join("test.webp");
        assert!(file_path.exists(), "file should be created");
        assert_eq!(std::fs::read(&file_path).unwrap(), data);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_save_creates_nested_directory() {
        let dir = std::env::temp_dir()
            .join("mdpaste_local_test_nested")
            .join("images");
        let dir_str = dir.to_str().unwrap().to_string();
        let backend = LocalBackend::new(&dir_str);

        backend.save(b"data", "img.png").await.unwrap();
        assert!(dir.join("img.png").exists());

        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("mdpaste_local_test_nested"));
    }

    #[tokio::test]
    async fn test_save_url_format() {
        let dir = std::env::temp_dir().join("mdpaste_url_test");
        let dir_str = dir.to_str().unwrap().to_string();
        let backend = LocalBackend::new(&dir_str);

        let url = backend.save(b"x", "20260312_120000.webp").await.unwrap();
        assert_eq!(url, format!("./{dir_str}/20260312_120000.webp"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
