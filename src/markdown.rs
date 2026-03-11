/// Wrap a URL in a Markdown image tag: `![](url)`
pub fn generate(url: &str) -> String {
    format!("![]({})", url)
}
