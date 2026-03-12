use anyhow::Result;
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

        let creds = Credentials::new(
            &global.access_key,
            &global.secret_key,
            None,
            None,
            "mdpaste",
        );

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

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(image.to_vec()))
            .content_type("image/webp")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("R2 upload failed: {e}"))?;

        Ok(format!("{}/{}", self.public_url, key))
    }
}
