use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    config::{BehaviorVersion, Region},
    primitives::ByteStream,
    Client,
};

use crate::config::{R2GlobalConfig, R2ProjectConfig};

pub struct R2Backend {
    client: Client,
    bucket: String,
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

        let creds = Credentials::new(&access_key, &secret_key, None, None, "mdpaste");

        let conf = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(creds)
            .region(Region::new("auto"))
            .endpoint_url(endpoint)
            .force_path_style(true)
            .build();

        Ok(R2Backend {
            client: Client::from_conf(conf),
            bucket: project.bucket.clone(),
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
        let content_type = super::mime_for_ext(ext);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(image.to_vec()))
            .content_type(content_type)
            .send()
            .await
            .context("R2 upload failed")?;

        Ok(format!("{}/{}", self.public_url, key))
    }
}

#[cfg(test)]
mod tests {
    use super::super::mime_for_ext;

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
