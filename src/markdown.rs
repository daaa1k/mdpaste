/// Wrap a URL in a Markdown image tag: `![](url)`
pub fn generate(url: &str) -> String {
    format!("![]({})", url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_https_url() {
        assert_eq!(
            generate("https://example.com/img.webp"),
            "![](https://example.com/img.webp)"
        );
    }

    #[test]
    fn test_generate_relative_path() {
        assert_eq!(generate("./images/foo.webp"), "![](./images/foo.webp)");
    }

    #[test]
    fn test_generate_empty_url() {
        assert_eq!(generate(""), "![]()");
    }

    #[test]
    fn test_generate_with_spaces() {
        assert_eq!(generate("my image.webp"), "![](my image.webp)");
    }
}
