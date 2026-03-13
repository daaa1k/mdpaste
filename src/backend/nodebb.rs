use anyhow::{Context, Result};
use serde_json::Value;

pub struct NodebbBackend {
    client: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl NodebbBackend {
    pub async fn new(url: &str) -> Result<Self> {
        let username =
            std::env::var("NODEBB_USERNAME").context("NODEBB_USERNAME env var not set")?;
        let password =
            std::env::var("NODEBB_PASSWORD").context("NODEBB_PASSWORD env var not set")?;
        Self::new_inner(url, username, password)
    }

    fn new_inner(url: &str, username: String, password: String) -> Result<Self> {
        let client = reqwest::Client::builder().cookie_store(true).build()?;
        Ok(Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
            username,
            password,
        })
    }

    /// Upload `image` bytes to NodeBB and return the public URL.
    ///
    /// Login flow:
    /// 1. GET  /api/config  → csrf_token
    /// 2. POST /login       → session cookie
    /// 3. GET  /api/config  → refreshed csrf_token
    /// 4. POST /api/post/upload (multipart)
    pub async fn save(&self, image: &[u8], filename: &str) -> Result<String> {
        let csrf1 = self.fetch_csrf().await?;
        self.login(&csrf1).await?;
        let csrf2 = self.fetch_csrf().await?;

        let mime = mime_for_filename(filename);
        let part = reqwest::multipart::Part::bytes(image.to_vec())
            .file_name(filename.to_string())
            .mime_str(mime)?;
        let form = reqwest::multipart::Form::new().part("files[]", part);

        let res = self
            .client
            .post(format!("{}/api/post/upload", self.base_url))
            .header("x-csrf-token", csrf2)
            .multipart(form)
            .send()
            .await?;

        let status = res.status();
        let body = res
            .text()
            .await
            .context("failed to read NodeBB upload response body")?;

        if !status.is_success() {
            anyhow::bail!("NodeBB upload failed: HTTP {} — {}", status, body);
        }

        let json: Value = serde_json::from_str(&body)
            .with_context(|| format!("error decoding response body: {body}"))?;

        // NodeBB v3 wraps results: {"status":{...},"response":{"images":[{"url":"..."}]}}
        // Fall back to legacy flat array format: [{"url":"..."}]
        let rel_url = json
            .pointer("/response/images/0/url")
            .or_else(|| json.get(0).and_then(|v| v.get("url")))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("url field missing in NodeBB upload response: {body}")
            })?;

        if rel_url.starts_with("http") {
            Ok(rel_url.to_string())
        } else {
            // Relative URLs from NodeBB are root-relative (e.g. /forum/assets/...),
            // so resolve against the origin only, not the full base_url path.
            Ok(format!("{}{}", origin_of(&self.base_url), rel_url))
        }
    }

    async fn fetch_csrf(&self) -> Result<String> {
        let json: Value = self
            .client
            .get(format!("{}/api/config", self.base_url))
            .send()
            .await?
            .json()
            .await?;
        json["csrf_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("csrf_token not found in /api/config"))
            .map(str::to_string)
    }

    async fn login(&self, csrf: &str) -> Result<()> {
        let res = self
            .client
            .post(format!("{}/login", self.base_url))
            .header("x-csrf-token", csrf)
            .form(&[
                ("username", self.username.as_str()),
                ("password", self.password.as_str()),
                ("noscript", "true"),
            ])
            .send()
            .await?;

        if !res.status().is_success() {
            anyhow::bail!("NodeBB login failed: HTTP {}", res.status());
        }
        Ok(())
    }
}

/// Extract the origin (`scheme://host:port`) from a URL, stripping any path.
///
/// ```
/// # use mdpaste::backend::nodebb::origin_of; // illustrative — fn is private
/// ```
///
/// Examples:
/// - `"https://example.com/forum"` → `"https://example.com"`
/// - `"http://127.0.0.1:1234"`     → `"http://127.0.0.1:1234"`
fn origin_of(url: &str) -> &str {
    // Skip past "://" to find the authority start.
    let after_scheme = url.find("://").map(|i| i + 3).unwrap_or(0);
    // The first '/' after the authority marks where the path begins.
    match url[after_scheme..].find('/') {
        Some(path_start) => &url[..after_scheme + path_start],
        None => url,
    }
}

/// Map a filename's extension to its MIME type.
fn mime_for_filename(filename: &str) -> &'static str {
    match filename.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/webp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn make_backend(url: &str) -> NodebbBackend {
        NodebbBackend::new_inner(url, "testuser".to_string(), "testpass".to_string()).unwrap()
    }

    // ── origin_of ─────────────────────────────────────────────────────────────

    #[test]
    fn test_origin_of_strips_path() {
        assert_eq!(
            origin_of("https://example.com/forum"),
            "https://example.com"
        );
    }

    #[test]
    fn test_origin_of_strips_deep_path() {
        assert_eq!(
            origin_of("https://example.com/a/b/c"),
            "https://example.com"
        );
    }

    #[test]
    fn test_origin_of_no_path_unchanged() {
        assert_eq!(origin_of("https://example.com"), "https://example.com");
    }

    #[test]
    fn test_origin_of_with_port() {
        assert_eq!(
            origin_of("http://127.0.0.1:1234/path"),
            "http://127.0.0.1:1234"
        );
    }

    #[test]
    fn test_origin_of_no_scheme() {
        // Gracefully handles a URL without "://".
        assert_eq!(origin_of("example.com/path"), "example.com");
    }

    // ── mime_for_filename ─────────────────────────────────────────────────────

    #[test]
    fn test_mime_for_filename_known_types() {
        assert_eq!(mime_for_filename("image.png"), "image/png");
        assert_eq!(mime_for_filename("photo.jpg"), "image/jpeg");
        assert_eq!(mime_for_filename("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_for_filename("anim.gif"), "image/gif");
        assert_eq!(mime_for_filename("screenshot.webp"), "image/webp");
    }

    #[test]
    fn test_mime_for_filename_unknown_defaults_to_webp() {
        assert_eq!(mime_for_filename("file.bmp"), "image/webp");
        assert_eq!(mime_for_filename("file"), "image/webp");
    }

    // ── fetch_csrf ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_csrf_success() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"tok123"}"#)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let token = backend.fetch_csrf().await.unwrap();
        assert_eq!(token, "tok123");
    }

    #[tokio::test]
    async fn test_fetch_csrf_missing_field_returns_error() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"version":"3.11.1"}"#)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let err = backend.fetch_csrf().await.unwrap_err();
        assert!(err.to_string().contains("csrf_token not found"));
    }

    // ── login ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_login_success() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("POST", "/login")
            .with_status(200)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        backend.login("csrf-tok").await.unwrap();
    }

    #[tokio::test]
    async fn test_login_failure_returns_error() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("POST", "/login")
            .with_status(403)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let err = backend.login("csrf-tok").await.unwrap_err();
        assert!(err.to_string().contains("NodeBB login failed"));
    }

    // ── save ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_save_with_relative_url() {
        let mut server = Server::new_async().await;
        let base = server.url();

        let _m1 = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"token1"}"#)
            .expect(2)
            .create_async()
            .await;
        let _m2 = server
            .mock("POST", "/login")
            .with_status(200)
            .create_async()
            .await;
        let _m3 = server
            .mock("POST", "/api/post/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":{"code":"ok","message":"OK"},"response":{"images":[{"url":"/assets/uploads/test.webp","name":"test.webp"}]}}"#)
            .create_async()
            .await;

        let backend = make_backend(&base);
        let url = backend.save(b"fakeimage", "test.webp").await.unwrap();
        assert_eq!(url, format!("{base}/assets/uploads/test.webp"));
    }

    #[tokio::test]
    async fn test_save_with_absolute_url() {
        let mut server = Server::new_async().await;

        let _m1 = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"token1"}"#)
            .expect(2)
            .create_async()
            .await;
        let _m2 = server
            .mock("POST", "/login")
            .with_status(200)
            .create_async()
            .await;
        let _m3 = server
            .mock("POST", "/api/post/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":{"code":"ok","message":"OK"},"response":{"images":[{"url":"https://cdn.example.com/uploads/test.png","name":"test.png"}]}}"#)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let url = backend.save(b"fakeimage", "test.png").await.unwrap();
        assert_eq!(url, "https://cdn.example.com/uploads/test.png");
    }

    #[tokio::test]
    async fn test_save_with_base_url_path() {
        // Regression: when base_url has a path component (e.g. /forum), the
        // relative upload URL (/forum/assets/...) must NOT produce a doubled
        // path like /forum/forum/assets/...
        let mut server = Server::new_async().await;
        let base_with_path = format!("{}/forum", server.url());

        let _m1 = server
            .mock("GET", "/forum/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"token1"}"#)
            .expect(2)
            .create_async()
            .await;
        let _m2 = server
            .mock("POST", "/forum/login")
            .with_status(200)
            .create_async()
            .await;
        let _m3 = server
            .mock("POST", "/forum/api/post/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":{"code":"ok","message":"OK"},"response":{"images":[{"url":"/forum/assets/uploads/files/test.png","name":"test.png"}]}}"#)
            .create_async()
            .await;

        let backend = make_backend(&base_with_path);
        let url = backend.save(b"fakeimage", "test.png").await.unwrap();
        // Must be origin + rel_url, not base_url + rel_url
        let expected = format!("{}/forum/assets/uploads/files/test.png", server.url());
        assert_eq!(
            url, expected,
            "URL must not duplicate the /forum path segment"
        );
    }

    #[tokio::test]
    async fn test_save_upload_failure_returns_error() {
        let mut server = Server::new_async().await;

        let _m1 = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"token1"}"#)
            .expect(2)
            .create_async()
            .await;
        let _m2 = server
            .mock("POST", "/login")
            .with_status(200)
            .create_async()
            .await;
        let _m3 = server
            .mock("POST", "/api/post/upload")
            .with_status(500)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let err = backend.save(b"fakeimage", "test.webp").await.unwrap_err();
        assert!(err.to_string().contains("NodeBB upload failed"));
    }
}
