use anyhow::{Context, Result};
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use std::time::Duration;

use crate::config::{R2GlobalConfig, R2ProjectConfig};

pub struct R2Backend {
    bucket: Bucket,
    credentials: Credentials,
    client: reqwest::Client,
    public_url: String,
    prefix: String,
}

impl R2Backend {
    pub async fn new(global: &R2GlobalConfig, project: &R2ProjectConfig) -> Result<Self> {
        let endpoint = global
            .endpoint
            .clone()
            .unwrap_or_else(|| format!("https://{}.r2.cloudflarestorage.com", global.account_id));

        let access_key = std::env::var("R2_ACCESS_KEY_ID")
            .context("R2_ACCESS_KEY_ID environment variable not set")?;
        let secret_key = std::env::var("R2_SECRET_ACCESS_KEY")
            .context("R2_SECRET_ACCESS_KEY environment variable not set")?;

        let endpoint_url: url::Url = endpoint.parse().context("Invalid R2 endpoint URL")?;
        let bucket = Bucket::new(endpoint_url, UrlStyle::Path, project.bucket.clone(), "auto")
            .context("Failed to initialise R2 bucket")?;

        let credentials = Credentials::new(access_key, secret_key);

        Ok(R2Backend {
            bucket,
            credentials,
            client: reqwest::Client::new(),
            public_url: project.public_url.trim_end_matches('/').to_string(),
            prefix: project.prefix.clone().unwrap_or_default(),
        })
    }

    /// Upload `image` bytes to R2 and return the public URL.
    pub async fn save(&self, image: &[u8], filename: &str) -> Result<String> {
        let key = format!("{}{}", self.prefix, filename);
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("webp");
        let content_type = mime_for_ext(ext);

        let action = self.bucket.put_object(Some(&self.credentials), &key);
        let signed_url = action.sign(Duration::from_secs(3600));

        self.client
            .put(signed_url)
            .header("Content-Type", content_type)
            .body(image.to_vec())
            .send()
            .await
            .context("R2 upload request failed")?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("R2 upload failed: {e}"))?;

        Ok(format!("{}/{}", self.public_url, key))
    }
}

/// Map a lowercase file extension to its MIME type.
fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/webp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_for_ext_known_types() {
        assert_eq!(mime_for_ext("png"), "image/png");
        assert_eq!(mime_for_ext("jpg"), "image/jpeg");
        assert_eq!(mime_for_ext("jpeg"), "image/jpeg");
        assert_eq!(mime_for_ext("gif"), "image/gif");
        assert_eq!(mime_for_ext("webp"), "image/webp");
    }

    #[test]
    fn test_mime_for_ext_unknown_defaults_to_webp() {
        assert_eq!(mime_for_ext("bmp"), "image/webp");
        assert_eq!(mime_for_ext("tiff"), "image/webp");
        assert_eq!(mime_for_ext(""), "image/webp");
    }
}
