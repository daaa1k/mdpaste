use anyhow::{Context, Result};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct NodebbBackend {
    client: reqwest::Client,
    cookie_store: Arc<CookieStoreMutex>,
    base_url: String,
    username: String,
    password: String,
    cookie_path: Option<PathBuf>,
    debug: bool,
}

impl NodebbBackend {
    pub async fn new(url: &str, debug: bool) -> Result<Self> {
        let username =
            std::env::var("NODEBB_USERNAME").context("NODEBB_USERNAME env var not set")?;
        let password =
            std::env::var("NODEBB_PASSWORD").context("NODEBB_PASSWORD env var not set")?;
        let cookie_path = Some(cookie_path_for_url(url));
        if debug {
            eprintln!(
                "[mdpaste debug] cookie path: {}",
                cookie_path.as_ref().unwrap().display()
            );
        }
        Self::new_inner(url, username, password, cookie_path, debug)
    }

    fn new_inner(
        url: &str,
        username: String,
        password: String,
        cookie_path: Option<PathBuf>,
        debug: bool,
    ) -> Result<Self> {
        let store = match &cookie_path {
            Some(path) if path.exists() => {
                if debug {
                    eprintln!("[mdpaste debug] cookie file found: {}", path.display());
                }
                load_cookie_store(path, debug)
            }
            _ => CookieStore::default(),
        };
        let cookie_store = Arc::new(CookieStoreMutex::new(store));
        let client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_store))
            .build()?;
        Ok(Self {
            client,
            cookie_store,
            base_url: url.trim_end_matches('/').to_string(),
            username,
            password,
            cookie_path,
            debug,
        })
    }

    /// Upload `image` bytes to NodeBB and return the public URL.
    ///
    /// Login flow:
    /// - If a cached session cookie exists and is valid (uid > 0), skip login.
    /// - Otherwise: GET /api/config → csrf, POST /login, save cookies.
    /// - GET /api/config → csrf for upload.
    /// - POST /api/post/upload (multipart).
    pub async fn save(&self, image: &[u8], filename: &str) -> Result<String> {
        let csrf = self.ensure_logged_in().await?;

        let mime = mime_for_filename(filename);
        let part = reqwest::multipart::Part::bytes(image.to_vec())
            .file_name(filename.to_string())
            .mime_str(mime)?;
        let form = reqwest::multipart::Form::new().part("files[]", part);

        let res = self
            .client
            .post(format!("{}/api/post/upload", self.base_url))
            .header("x-csrf-token", csrf)
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

    /// Ensures the client has a valid session and returns a CSRF token for upload.
    ///
    /// Checks `/api/config` for `uid > 0` at the top level (logged-in indicator).  If the
    /// cached session is still valid the same response's `csrf_token` is
    /// returned immediately.  Otherwise the client logs in and fetches a fresh
    /// token.
    async fn ensure_logged_in(&self) -> Result<String> {
        if self.debug {
            let n = self
                .cookie_store
                .lock()
                .expect("poisoned")
                .iter_any()
                .count();
            eprintln!("[mdpaste debug] {n} cookie(s) in store before GET /api/config");
        }

        let json = self.get_config_json().await?;
        // NodeBB returns `uid` at the top level (not nested under `user`).
        // `loggedIn` is also available but uid > 0 is the canonical check.
        let uid = json["uid"].as_u64().unwrap_or(0);
        let csrf = extract_csrf(&json)?;

        if self.debug {
            eprintln!("[mdpaste debug] /api/config call completed");
        }

        if uid > 0 {
            if self.debug {
                eprintln!("[mdpaste debug] session valid, skipping login");
            }
            return Ok(csrf);
        }

        if self.debug {
            eprintln!("[mdpaste debug] not logged in, authenticating");
        }
        // Not logged in: use the csrf we already fetched to authenticate.
        self.login(&csrf).await?;
        self.save_cookies()?;

        // Fetch a session-bound csrf for the upload request.
        let json2 = self.get_config_json().await?;
        extract_csrf(&json2)
    }

    async fn get_config_json(&self) -> Result<Value> {
        self.client
            .get(format!("{}/api/config", self.base_url))
            .send()
            .await?
            .json()
            .await
            .context("failed to fetch /api/config")
    }

    /// Exposed for unit tests.
    #[cfg(test)]
    pub(crate) async fn fetch_csrf(&self) -> Result<String> {
        extract_csrf(&self.get_config_json().await?)
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

    fn save_cookies(&self) -> Result<()> {
        let Some(path) = &self.cookie_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create cache dir: {}", parent.display()))?;
        }
        let mut file = std::fs::File::create(path)
            .with_context(|| format!("failed to create cookie file: {}", path.display()))?;
        let store = self
            .cookie_store
            .lock()
            .expect("cookie store mutex poisoned");
        if self.debug {
            eprintln!(
                "[mdpaste debug] saving {} cookie(s) to {}",
                store.iter_any().count(),
                path.display()
            );
        }
        // Use the library's dedicated API which persists all cookies including
        // session cookies (no Expires/Max-Age).  The legacy CookieStore::Serialize
        // impl only writes persistent cookies, causing an empty file for NodeBB.
        cookie_store::serde::json::save_incl_expired_and_nonpersistent(&store, &mut file)
            .map_err(|e| anyhow::anyhow!("failed to save cookies: {e}"))
    }
}

// ── Cookie persistence helpers ────────────────────────────────────────────────

fn load_cookie_store(path: &Path, debug: bool) -> CookieStore {
    let Ok(file) = std::fs::File::open(path) else {
        return CookieStore::default();
    };
    let reader = std::io::BufReader::new(file);
    // Use cookie_store::serde::json::load which reads all non-expired cookies
    // including session cookies, using the same format as save_incl_expired_and_nonpersistent.
    match cookie_store::serde::json::load(reader) {
        Ok(store) => {
            if debug {
                let count = store.iter_any().count();
                eprintln!(
                    "[mdpaste debug] loaded {} cookie(s) from {}",
                    count,
                    path.display()
                );
            }
            store
        }
        Err(e) => {
            if debug {
                eprintln!("[mdpaste debug] failed to load cookies: {e}, starting fresh");
            }
            CookieStore::default()
        }
    }
}

fn cookie_path_for_url(url: &str) -> PathBuf {
    let key = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .replace(['/', ':', '.'], "_");
    cache_dir().join(format!("nodebb_{key}.json"))
}

fn cache_dir() -> PathBuf {
    // Windows: use %LOCALAPPDATA%\mdpaste
    #[cfg(target_os = "windows")]
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(dir).join("mdpaste");
    }

    // Unix: respect XDG_CACHE_HOME; fall back to ~/.cache
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(dir).join("mdpaste")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache").join("mdpaste")
    } else {
        PathBuf::from(".cache").join("mdpaste")
    }
}

// ── Misc helpers ──────────────────────────────────────────────────────────────

fn extract_csrf(json: &Value) -> Result<String> {
    json["csrf_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("csrf_token not found in /api/config"))
        .map(str::to_string)
}

/// Extract the origin (`scheme://host:port`) from a URL, stripping any path.
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
        NodebbBackend::new_inner(
            url,
            "testuser".to_string(),
            "testpass".to_string(),
            None,
            false,
        )
        .unwrap()
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

    // ── session cookie persistence ────────────────────────────────────────────

    /// Session cookies (no Expires/Max-Age) must survive a save→load roundtrip.
    ///
    /// This guards against the `CookieStore::Serialize` legacy impl which filters
    /// out session cookies via `is_persistent()`.
    #[test]
    fn test_session_cookie_save_load_roundtrip() {
        let json = concat!(
            r#"[{"raw_cookie":"sid=abc123; HttpOnly; SameSite=Lax; Path=/forum","#,
            r#""path":["/forum",true],"domain":{"HostOnly":"example.com"},"expires":"SessionEnd"}]"#,
            "\n",
        );
        // load must successfully parse a session cookie
        let store =
            cookie_store::serde::json::load(json.as_bytes()).expect("session cookie load failed");
        assert_eq!(store.iter_any().count(), 1, "session cookie must be loaded");

        // save_incl_expired_and_nonpersistent must round-trip it
        let mut buf = Vec::new();
        cookie_store::serde::json::save_incl_expired_and_nonpersistent(&store, &mut buf)
            .expect("session cookie save failed");
        let saved = String::from_utf8(buf).unwrap();

        let store2 =
            cookie_store::serde::json::load(saved.as_bytes()).expect("reloaded session cookie");
        assert_eq!(
            store2.iter_any().count(),
            1,
            "session cookie must survive save→load"
        );
    }

    // ── cookie_path_for_url ───────────────────────────────────────────────────

    #[test]
    fn test_cookie_path_for_url_sanitises_https() {
        let path = cookie_path_for_url("https://forum.example.com/forum");
        let name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "nodebb_forum_example_com_forum.json");
    }

    #[test]
    fn test_cookie_path_for_url_sanitises_localhost_port() {
        let path = cookie_path_for_url("http://localhost:4567");
        let name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "nodebb_localhost_4567.json");
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

    /// Helper: set up the standard "not logged in" mock sequence.
    /// /api/config called twice (check session + post-login csrf), /login once.
    async fn mock_login_sequence(server: &mut Server, csrf: &str) -> Vec<mockito::Mock> {
        let m_config = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"csrf_token":"{csrf}"}}"#))
            .expect(2)
            .create_async()
            .await;
        let m_login = server
            .mock("POST", "/login")
            .with_status(200)
            .create_async()
            .await;
        vec![m_config, m_login]
    }

    #[tokio::test]
    async fn test_save_with_relative_url() {
        let mut server = Server::new_async().await;
        let base = server.url();

        let _mocks = mock_login_sequence(&mut server, "token1").await;
        let _m_upload = server
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

        let _mocks = mock_login_sequence(&mut server, "token1").await;
        let _m_upload = server
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

        let _m_config = server
            .mock("GET", "/forum/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"token1"}"#)
            .expect(2)
            .create_async()
            .await;
        let _m_login = server
            .mock("POST", "/forum/login")
            .with_status(200)
            .create_async()
            .await;
        let _m_upload = server
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

        let _mocks = mock_login_sequence(&mut server, "token1").await;
        let _m_upload = server
            .mock("POST", "/api/post/upload")
            .with_status(500)
            .create_async()
            .await;

        let backend = make_backend(&server.url());
        let err = backend.save(b"fakeimage", "test.webp").await.unwrap_err();
        assert!(err.to_string().contains("NodeBB upload failed"));
    }

    /// When /api/config reports uid > 0, login must be skipped entirely.
    #[tokio::test]
    async fn test_save_skips_login_when_session_valid() {
        let mut server = Server::new_async().await;
        let base = server.url();

        // /api/config returns a logged-in user → only called once
        let _m_config = server
            .mock("GET", "/api/config")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"csrf_token":"tok42","uid":1,"loggedIn":true}"#)
            .expect(1)
            .create_async()
            .await;
        // /login must NOT be called
        let m_login = server.mock("POST", "/login").expect(0).create_async().await;
        let _m_upload = server
            .mock("POST", "/api/post/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"status":{"code":"ok"},"response":{"images":[{"url":"/assets/img.png"}]}}"#,
            )
            .create_async()
            .await;

        let backend = make_backend(&base);
        let url = backend.save(b"img", "img.png").await.unwrap();
        assert_eq!(url, format!("{base}/assets/img.png"));
        m_login.assert_async().await;
    }
}
