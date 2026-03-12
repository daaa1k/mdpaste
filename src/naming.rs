use chrono::Local;

/// Generate a timestamp-based filename, e.g. `20260312_120233.webp`.
pub fn generate_filename(ext: &str) -> String {
    format!("{}.{}", Local::now().format("%Y%m%d_%H%M%S"), ext)
}

/// Generate a timestamp-based filename with a sequence number suffix,
/// e.g. `20260312_120233_2.webp`.  Used when uploading multiple files to
/// avoid name collisions within the same second.
pub fn generate_filename_n(n: usize, ext: &str) -> String {
    format!("{}_{n}.{}", Local::now().format("%Y%m%d_%H%M%S"), ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename_webp() {
        let name = generate_filename("webp");
        assert!(
            name.ends_with(".webp"),
            "expected .webp suffix, got: {name}"
        );
        // timestamp part: YYYYMMDD_HHMMSS = 15 chars
        let base = name.strip_suffix(".webp").unwrap();
        assert_eq!(base.len(), 15, "timestamp should be 15 chars, got: {base}");
    }

    #[test]
    fn test_generate_filename_png() {
        let name = generate_filename("png");
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn test_generate_filename_gif() {
        let name = generate_filename("gif");
        assert!(name.ends_with(".gif"));
    }

    #[test]
    fn test_generate_filename_n_index() {
        let name = generate_filename_n(1, "webp");
        assert!(name.ends_with("_1.webp"), "got: {name}");

        let name3 = generate_filename_n(3, "png");
        assert!(name3.ends_with("_3.png"), "got: {name3}");
    }

    #[test]
    fn test_generate_filename_n_different_extensions() {
        let name = generate_filename_n(2, "gif");
        assert!(name.ends_with("_2.gif"));
    }
}
